//! On-demand cameras. Encoders and JPEG frames are shared by all dashboard viewers.
//! Frames stay in memory; no continuous writes to the Pi's SD card.
use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, Once};
use std::time::{Duration, Instant};

const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
const FRAME_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_JPEG_BYTES: usize = 4 * 1024 * 1024;
const MAX_VIEWERS: usize = 4;

struct FrameState {
    jpeg: Arc<Vec<u8>>,
    sequence: u64,
    timestamp: i64,
    received: Option<Instant>,
    demand: Option<Instant>,
    viewers: usize,
    running: bool,
    error: Option<String>,
}
struct Feed {
    id: &'static str,
    label: &'static str,
    state: Mutex<FrameState>,
    ready: Condvar,
    child: Mutex<Option<Child>>,
}
impl Feed {
    fn new(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            state: Mutex::new(FrameState {
                jpeg: Arc::new(Vec::new()),
                sequence: 0,
                timestamp: 0,
                received: None,
                demand: None,
                viewers: 0,
                running: false,
                error: None,
            }),
            ready: Condvar::new(),
            child: Mutex::new(None),
        }
    }
    fn wanted(&self) -> bool {
        !STOPPED.load(Ordering::Relaxed)
            && self
                .state
                .lock()
                .unwrap()
                .demand
                .is_some_and(|demand| demand.elapsed() < IDLE_TIMEOUT)
    }
    fn stop(&self) {
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let mut state = self.state.lock().unwrap();
        state.running = false;
        state.received = None;
        state.jpeg = Arc::new(Vec::new());
        self.ready.notify_all();
    }
}
lazy_static::lazy_static! {
    static ref FEEDS: [Arc<Feed>; 2] = [
        Arc::new(Feed::new("ribbon", "Raspberry Pi ribbon camera")),
        Arc::new(Feed::new("usb", "USB webcam")),
    ];
}
static START: Once = Once::new();
static STOPPED: AtomicBool = AtomicBool::new(false);

fn executable(names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .flat_map(|name| ["/usr/bin", "/usr/local/bin"].map(|dir| Path::new(dir).join(name)))
        .find(|path| path.is_file())
}
fn usb_device() -> Option<PathBuf> {
    // Stable USB identity, never the adjacent UVC metadata node or Pi codecs.
    let mut candidates: Vec<_> = fs::read_dir("/dev/v4l/by-id")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| name.starts_with("usb-") && name.ends_with("-video-index0"))
        })
        .collect();
    candidates.sort();
    if let Some(path) = candidates.into_iter().find(|path| path.exists()) {
        return Some(path);
    }
    let mut nodes: Vec<_> = fs::read_dir("/sys/class/video4linux")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    nodes.sort();
    nodes.into_iter().find_map(|path| {
        let device = fs::canonicalize(path.join("device")).ok()?;
        let index = fs::read_to_string(path.join("index")).ok()?;
        (device.to_string_lossy().contains("/usb") && index.trim() == "0")
            .then(|| Path::new("/dev").join(path.file_name().unwrap()))
    })
}
fn capture_command(id: &str) -> Result<Command, String> {
    if id == "ribbon" {
        let binary = executable(&["rpicam-vid", "libcamera-vid"])
            .ok_or("Install rpicam-apps-lite to use the ribbon camera.")?;
        let mut command = Command::new(binary);
        command.args([
            "--camera",
            "0",
            "--nopreview",
            "--timeout",
            "0",
            "--width",
            "640",
            "--height",
            "480",
            "--framerate",
            "10",
            "--codec",
            "mjpeg",
            "--quality",
            "80",
            "--output",
            "-",
        ]);
        Ok(command)
    } else {
        let binary = executable(&["ffmpeg"]).ok_or("Install ffmpeg to use the USB webcam.")?;
        let device = usb_device().ok_or("USB webcam not detected. Check its USB connection.")?;
        let mut command = Command::new(binary);
        // Let V4L2 select the input pixel format: many webcams only offer YUYV.
        command
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "v4l2",
                "-framerate",
                "10",
                "-video_size",
                "640x480",
                "-i",
            ])
            .arg(device)
            .args([
                "-an",
                "-vf",
                "fps=10",
                "-c:v",
                "mjpeg",
                "-threads",
                "1",
                "-q:v",
                "5",
                "-f",
                "image2pipe",
                "pipe:1",
            ]);
        Ok(command)
    }
}
fn capture(feed: &Arc<Feed>) -> Result<(), String> {
    let mut command = capture_command(feed.id)?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Cannot start camera: {error}"))?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let diagnostics = Arc::new(Mutex::new(Vec::new()));
    let diagnostic_copy = Arc::clone(&diagnostics);
    // Drain diagnostics continuously, retaining only a bounded tail.
    let error_reader = std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        while let Ok(count) = stderr.read(&mut buf) {
            if count == 0 {
                break;
            }
            let mut tail = diagnostic_copy.lock().unwrap();
            tail.extend_from_slice(&buf[..count]);
            let excess = tail.len().saturating_sub(2048);
            tail.drain(..excess);
        }
    });
    *feed.child.lock().unwrap() = Some(child);
    if STOPPED.load(Ordering::Relaxed) {
        feed.stop();
        let _ = error_reader.join();
        return Ok(());
    }
    {
        let mut state = feed.state.lock().unwrap();
        state.running = true;
        state.error = None;
    }
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 16384];
    let mut capture_error = None;
    'frames: loop {
        let count = match stdout.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                capture_error = Some(format!("Camera stopped: {error}"));
                break;
            }
        };
        buffer.extend_from_slice(&chunk[..count]);
        loop {
            let Some(start) = buffer.windows(2).position(|bytes| bytes == [0xff, 0xd8]) else {
                if buffer.len() > 1 {
                    buffer.drain(..buffer.len() - 1);
                }
                break;
            };
            if start > 0 {
                buffer.drain(..start);
            }
            let Some(end) = buffer[2..]
                .windows(2)
                .position(|bytes| bytes == [0xff, 0xd9])
                .map(|index| index + 4)
            else {
                break;
            };
            if end > MAX_JPEG_BYTES {
                capture_error = Some("Camera produced an oversized JPEG frame.".into());
                break 'frames;
            }
            let jpeg = buffer.drain(..end).collect::<Vec<_>>();
            let mut state = feed.state.lock().unwrap();
            state.jpeg = Arc::new(jpeg);
            state.sequence = state.sequence.wrapping_add(1);
            state.timestamp = chrono::Utc::now().timestamp_millis();
            state.received = Some(Instant::now());
            state.error = None;
            feed.ready.notify_all();
        }
        if buffer.len() > MAX_JPEG_BYTES {
            capture_error = Some("Camera produced an oversized or incomplete JPEG frame.".into());
            break;
        }
        if !feed.wanted() {
            break;
        }
    }
    feed.stop();
    let _ = error_reader.join();
    let tail = String::from_utf8_lossy(&diagnostics.lock().unwrap())
        .trim()
        .to_string();
    if !feed.wanted() {
        return Ok(());
    }
    Err(capture_error.unwrap_or_else(|| {
        if tail.is_empty() {
            "No video frames received. Check the camera connection and retry.".into()
        } else {
            tail
        }
    }))
}
pub fn init_all() {
    START.call_once(|| {
        for feed in FEEDS.iter() {
            let worker = Arc::clone(feed);
            std::thread::spawn(move || {
                while !STOPPED.load(Ordering::Relaxed) {
                    if worker.wanted() {
                        if let Err(error) = capture(&worker) {
                            log::warn!("{}: {}", worker.label, error);
                            worker.state.lock().unwrap().error = Some(error);
                            worker.ready.notify_all();
                            for _ in 0..5 {
                                if !worker.wanted() {
                                    break;
                                }
                                std::thread::sleep(Duration::from_secs(1));
                            }
                        }
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            });
            let watcher = Arc::clone(feed);
            std::thread::spawn(move || {
                let mut started = Instant::now();
                while !STOPPED.load(Ordering::Relaxed) {
                    let state = watcher.state.lock().unwrap();
                    let stalled = state.running
                        && state.received.unwrap_or(started).elapsed() > FRAME_TIMEOUT;
                    if !state.running {
                        started = Instant::now();
                    }
                    drop(state);
                    if !watcher.wanted() || stalled {
                        watcher.stop();
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            });
        }
    });
}
pub fn shutdown() {
    STOPPED.store(true, Ordering::Relaxed);
    for feed in FEEDS.iter() {
        feed.stop();
    }
}
pub fn status() -> serde_json::Value {
    serde_json::json!(FEEDS.iter().map(|feed| {
        let state = feed.state.lock().unwrap();
        let available = if feed.id == "usb" { usb_device().is_some() } else {
            // A detected CSI sensor exposes a V4L2 subdevice (Pi 4/5).
            fs::read_dir("/dev").ok().into_iter().flatten().filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with("v4l-subdev"))
        };
        let error = state.error.clone().or_else(|| capture_command(feed.id).err());
        serde_json::json!({"id": feed.id, "label": feed.label, "detected": available,
            "stream_url": format!("/api/cameras/{}/stream", feed.id),
            "live": state.running && state.received.is_some_and(|time| time.elapsed() < Duration::from_secs(3)),
            "running": state.running, "timestamp_ms": state.timestamp,
            "error": error, "viewers": state.viewers, "width": 640, "height": 480, "fps": 10})
    }).collect::<Vec<_>>())
}
pub struct CameraStream {
    feed: Arc<Feed>,
    current: Cursor<Vec<u8>>,
    sequence: u64,
    authorized: Box<dyn Fn() -> bool + Send>,
    opened: Instant,
}
pub fn stream(id: &str, authorized: Box<dyn Fn() -> bool + Send>) -> Result<CameraStream, String> {
    let feed = FEEDS
        .iter()
        .find(|feed| feed.id == id)
        .ok_or("Unknown camera")?;
    capture_command(id)?;
    let mut state = feed.state.lock().unwrap();
    if state.viewers >= MAX_VIEWERS {
        return Err("This camera already has four viewers. Close another view and retry.".into());
    }
    state.viewers += 1;
    state.demand = Some(Instant::now());
    Ok(CameraStream {
        feed: Arc::clone(feed),
        current: Cursor::new(Vec::new()),
        sequence: state.sequence,
        authorized,
        opened: Instant::now(),
    })
}
impl Read for CameraStream {
    fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
        if target.is_empty() {
            return Ok(0);
        }
        let count = self.current.read(target)?;
        if count > 0 {
            return Ok(count);
        }
        // Renew the connection periodically, and revoke it immediately on logout,
        // password changes, or session expiry, even if frames are still arriving.
        if self.opened.elapsed() > Duration::from_secs(300) || !(self.authorized)() {
            return Ok(0);
        }
        let deadline = Instant::now() + FRAME_TIMEOUT;
        loop {
            if STOPPED.load(Ordering::Relaxed) || !(self.authorized)() {
                return Ok(0);
            }
            let mut state = self.feed.state.lock().unwrap();
            state.demand = Some(Instant::now());
            if state.sequence != self.sequence
                && state
                    .received
                    .is_some_and(|time| time.elapsed() < Duration::from_secs(3))
            {
                self.sequence = state.sequence;
                let jpeg = Arc::clone(&state.jpeg);
                drop(state);
                let mut bytes = format!(
                    "--aog-frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    jpeg.len()
                )
                .into_bytes();
                bytes.extend_from_slice(&jpeg);
                bytes.extend_from_slice(b"\r\n");
                self.current = Cursor::new(bytes);
                return self.current.read(target);
            }
            if Instant::now() >= deadline {
                return Ok(0);
            }
            drop(
                self.feed
                    .ready
                    .wait_timeout(state, Duration::from_secs(1))
                    .unwrap(),
            );
        }
    }
}
impl Drop for CameraStream {
    fn drop(&mut self) {
        let mut state = self.feed.state.lock().unwrap();
        state.viewers = state.viewers.saturating_sub(1);
        if state.viewers == 0 {
            state.demand = None;
        }
    }
}
