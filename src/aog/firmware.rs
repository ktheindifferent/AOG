//! Packaged, checksummed firmware updates. Only identified AOG ports are writable.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

static ACTIVE: AtomicBool = AtomicBool::new(false);
lazy_static::lazy_static! {
    static ref JOB: Mutex<Job> = Mutex::new(Job::default());
}
#[derive(Clone, Default, Serialize)]
pub struct Job {
    pub device: String,
    pub stage: String,
    pub message: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Artifact {
    pub device: String,
    pub board: String,
    pub fqbn: String,
    pub file: String,
    pub sha256: String,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub version: String,
    pub protocol: u8,
    pub artifacts: Vec<Artifact>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateRequest {
    pub device: String,
    pub board: String,
}
pub fn active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}
pub fn holding(device: &str) -> bool {
    active() && JOB.lock().unwrap_or_else(|e| e.into_inner()).device == device
}
fn tool(directory: &Path) -> PathBuf {
    directory.join("tools/arduino-cli")
}
pub fn manifest(directory: &Path) -> Result<Manifest, String> {
    serde_json::from_slice(
        &fs::read(directory.join("firmware/manifest.json"))
            .map_err(|e| format!("Firmware package unavailable: {e}"))?,
    )
    .map_err(|e| e.to_string())
}
pub fn status(directory: &Path) -> serde_json::Value {
    let package = manifest(directory);
    serde_json::json!({"package":package.as_ref().ok(),"package_error":package.as_ref().err(),
        "updater_available":tool(directory).is_file(),"active":active(),"job":JOB.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        "repository":"https://github.com/PixelCoda/AOG", "releases":"https://github.com/PixelCoda/AOG/releases"})
}
fn checked_artifact(
    directory: &Path,
    request: &UpdateRequest,
) -> Result<(Artifact, PathBuf), String> {
    if !super::device_link::DEVICES.contains(&request.device.as_str())
        || !["nano", "nano-old", "uno"].contains(&request.board.as_str())
    {
        return Err("Choose an AOG board and a supported ATmega328P profile".into());
    }
    let package = manifest(directory)?;
    let artifact = package
        .artifacts
        .into_iter()
        .find(|a| a.device == request.device && a.board == request.board)
        .ok_or("This firmware package does not include that board")?;
    let fqbn = match request.board.as_str() {
        "nano" => "arduino:avr:nano:cpu=atmega328",
        "nano-old" => "arduino:avr:nano:cpu=atmega328old",
        _ => "arduino:avr:uno",
    };
    if artifact.fqbn != fqbn
        || artifact.file.contains('/')
        || artifact.file.contains('\\')
        || !artifact.file.ends_with(".hex")
    {
        return Err("Invalid firmware manifest".into());
    }
    let path = directory.join("firmware").join(&artifact.file);
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 131072 {
        return Err("Firmware file is too large".into());
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    if format!("{:x}", Sha256::digest(&bytes)) != artifact.sha256 {
        return Err("Firmware checksum failed; reinstall the AOG package".into());
    }
    Ok((artifact, path))
}
fn progress(stage: &str, message: impl Into<String>) {
    let mut job = JOB.lock().unwrap_or_else(|e| e.into_inner());
    job.stage = stage.into();
    job.message = message.into();
}
pub fn start(
    directory: &Path,
    request: UpdateRequest,
    outputs: Arc<Mutex<super::web_control::OutputController>>,
) -> Result<(), String> {
    let (artifact, hex) = checked_artifact(directory, &request)?;
    if !tool(directory).is_file() {
        return Err(
            "Install the Arduino upload tools with deploy/install-firmware-tools.sh first".into(),
        );
    }
    let board = super::device_link::statuses(directory)
        .into_iter()
        .find(|d| {
            d.device_id == request.device
                && (0..=8).contains(&(super::device_link::now() - d.observed_at))
        })
        .ok_or("The Arduino must be connected and identified before updating")?;
    let port = PathBuf::from(&board.port);
    let valid_port = port.parent() == Some(Path::new("/dev"))
        && port.file_name().and_then(|s| s.to_str()).is_some_and(|s| {
            ["ttyUSB", "ttyACM"].iter().any(|p| {
                s.strip_prefix(p)
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            })
        });
    if !valid_port {
        return Err("No valid serial port for this Arduino".into());
    }
    let node = fs::metadata(&port).map_err(|e| format!("Serial port unavailable: {e}"))?;
    if !node.file_type().is_char_device() {
        return Err("The serial port is not a device".into());
    }
    let port_identity = (node.dev(), node.ino(), node.rdev());
    if ACTIVE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A firmware update is already running".into());
    }
    *JOB.lock().unwrap_or_else(|e| e.into_inner()) = Job {
        device: request.device.clone(),
        stage: "preparing".into(),
        message: "Pausing schedules and checking outputs".into(),
        started_at: super::device_link::now(),
        finished_at: None,
    };
    let prepared: Result<(), String> = (|| {
        let mut control = outputs.lock().unwrap_or_else(|e| e.into_inner());
        control.pause_schedules()?;
        control.stop_all()?;
        if control.status().outputs.iter().any(|o| o.on != Some(false)) {
            return Err("All outputs must be confirmed off before updating firmware".into());
        }
        Ok(())
    })();
    if let Err(error) = prepared {
        finish(Err(error.clone()));
        return Err(error);
    }
    let directory = directory.to_owned();
    std::thread::Builder::new()
        .name("firmware-update".into())
        .spawn(move || {
            let result = upload(
                &directory,
                &request.device,
                &port,
                port_identity,
                &artifact,
                &hex,
                board
                    .fields
                    .get("PROTOCOL_VERSION")
                    .is_some_and(|s| s == "2"),
            );
            super::sensors::pause_port(&port, false);
            finish(result);
        })
        .map_err(|e| {
            let error = e.to_string();
            finish(Err(error.clone()));
            error
        })?;
    Ok(())
}
fn finish(result: Result<(), String>) {
    match result {Ok(())=>progress("complete","Firmware verified. Arduino reconnected; schedules remain paused until you resume them."),Err(e)=>progress("failed",e)};
    JOB.lock().unwrap_or_else(|e| e.into_inner()).finished_at = Some(super::device_link::now());
    ACTIVE.store(false, Ordering::SeqCst);
}
fn upload(
    directory: &Path,
    device: &str,
    port: &Path,
    port_identity: (u64, u64, u64),
    artifact: &Artifact,
    hex: &Path,
    buffered: bool,
) -> Result<(), String> {
    if buffered {
        progress(
            "draining",
            "Saving the Arduino’s buffered readings before reset",
        );
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let status = super::device_link::statuses(directory)
                .into_iter()
                .find(|s| s.device_id == device)
                .ok_or("Arduino disconnected")?;
            if !(0..=8).contains(&(super::device_link::now() - status.observed_at)) {
                return Err("Arduino disconnected before its logs were saved".into());
            }
            if status.fields.get("LOGGING").is_some_and(|s| s == "held")
                && status
                    .fields
                    .get("BUFFER_PENDING")
                    .is_some_and(|s| s == "0")
            {
                break;
            }
            if Instant::now() > deadline {
                return Err(
                    "Buffered readings could not be drained. Firmware was not flashed.".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    super::sensors::pause_port(port, true);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !super::sensors::port_idle(port) {
        if Instant::now() > deadline {
            return Err("Serial port did not become available".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // Reject a USB node replaced between identification and uploader handoff.
    let node = fs::metadata(port).map_err(|e| format!("Serial port disconnected: {e}"))?;
    if (node.dev(), node.ino(), node.rdev()) != port_identity {
        return Err("USB device changed before upload; reconnect and try again".into());
    }
    let identified = super::device_link::statuses(directory)
        .into_iter()
        .find(|s| {
            s.device_id == device
                && Path::new(&s.port) == port
                && (0..=8).contains(&(super::device_link::now() - s.observed_at))
        });
    if identified.is_none() {
        return Err("Arduino identity changed before upload".into());
    }
    progress(
        "flashing",
        "Writing and verifying the packaged firmware. Keep USB power connected.",
    );
    let log_path = directory.join("firmware-update.log");
    let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let mut command = Command::new(tool(directory));
    command
        .arg("--config-file")
        .arg(directory.join("tools/arduino-cli.yaml"))
        .args(["upload", "--fqbn", &artifact.fqbn, "--port"])
        .arg(port)
        .arg("--input-file")
        .arg(hex)
        .arg("--verify")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let successful = loop {
        if let Some(result) = child.try_wait().map_err(|e| e.to_string())? {
            break result.success();
        }
        if Instant::now() > deadline {
            // The CLI spawns avrdude. Stop its entire private process group
            // before releasing the serial port back to sensor discovery.
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "Upload timed out. Check the selected bootloader profile and USB connection."
                    .into(),
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    if !successful {
        return Err("Upload failed. Check the board profile and firmware-update.log; schedules remain paused.".into());
    }
    let flashed_at = super::device_link::now();
    super::sensors::pause_port(port, false);
    progress(
        "reconnecting",
        "Waiting for the updated Arduino to identify itself",
    );
    let expected = manifest(directory)?.version;
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline {
        if super::device_link::statuses(directory).iter().any(|s| {
            s.device_id == device
                && s.observed_at > flashed_at
                && s.fields.get("FIRMWARE_VERSION") == Some(&expected)
        }) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err("Upload verified, but the new firmware has not reconnected. Check USB; schedules remain paused.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_arbitrary_targets_before_opening_ports() {
        let d = tempfile::tempdir().unwrap();
        for board in ["../../etc/passwd", "nano;reboot", "arduino:avr:mega"] {
            assert!(checked_artifact(
                d.path(),
                &UpdateRequest {
                    device: "DUAL_OVF_SENSOR".into(),
                    board: board.into()
                }
            )
            .is_err());
        }
    }
    #[test]
    fn firmware_interlock_blocks_starts_and_resume_but_allows_stopping() {
        struct Board;
        impl super::super::web_control::RelayBoard for Board {
            fn states(&mut self) -> Result<[bool; 4], String> {
                Ok([false; 4])
            }
            fn set(&mut self, _: u8, on: bool) -> Result<(), String> {
                assert!(!on);
                Ok(())
            }
            fn all_off(&mut self) -> Result<(), String> {
                Ok(())
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let mut outputs = super::super::web_control::OutputController::new(
            directory.path(),
            crate::PumpConfig::default(),
            crate::DashboardConfig::default().relay_roles,
            Box::new(Board),
        );
        ACTIVE.store(true, Ordering::SeqCst);
        let start = outputs.start(1, None);
        let resume = outputs.resume_schedules();
        let stop = outputs.stop_all();
        ACTIVE.store(false, Ordering::SeqCst);
        assert!(start.unwrap_err().contains("Firmware update"));
        assert!(resume.is_err());
        assert!(stop.is_ok());
    }
}
