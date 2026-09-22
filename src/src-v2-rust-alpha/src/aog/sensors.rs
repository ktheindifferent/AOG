// Copyright (c) 2020-2021 Caleb Mitchell Smith (PixelCoda)
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
// THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use sds011::{Message, SDS011};
use serial2::SerialPort;
use std::collections::HashSet;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SENSOR_DIR: &str = "/opt/aog/sensors";
const OVERFLOW_DEVICE: &str = "DUAL_OVF_SENSOR";
const ENVIRONMENT_DEVICE: &str = "SENSORKIT_MK1";
// Allow Arduino reset/boot plus a second complete report after startup noise.
// The legacy 1200-baud kit needs about six seconds for this on a cold open.
const ARDUINO_BAUD_RATES: [u32; 2] = [9600, 1200];
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);
const STALE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_LINE_BYTES: usize = 512;
const MAX_FRAME_BYTES: usize = 2048;

lazy_static::lazy_static! {
    // All Arduino and particulate readers must claim a port before opening it.
    static ref CLAIMED_PORTS: Mutex<HashSet<PathBuf>> = Mutex::new(HashSet::new());
}

struct PortClaim(PathBuf);

impl PortClaim {
    fn acquire(path: &Path) -> Option<Self> {
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mut claimed = CLAIMED_PORTS.lock().unwrap_or_else(|e| e.into_inner());
        if claimed.insert(path.clone()) {
            Some(Self(path))
        } else {
            None
        }
    }
}

impl Drop for PortClaim {
    fn drop(&mut self) {
        CLAIMED_PORTS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.0);
    }
}

fn sensor_ports_in(directory: &Path) -> Vec<PathBuf> {
    let mut ports: Vec<_> = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            ["ttyUSB", "ttyACM"].iter().any(|prefix| {
                name.strip_prefix(prefix).is_some_and(|suffix| {
                    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
                })
            })
        })
        .map(|entry| entry.path())
        .collect();
    ports.sort();
    ports
}

pub fn init() {
    init_serial_monitoring();
    crate::aog::ph_sensor::init_ph_monitoring();
}

/// Start sensor readers without starting GPIO, relays, or other actuators.
pub fn init_serial_monitoring() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        fetch_arduino(OVERFLOW_DEVICE.to_owned());
        fetch_arduino(ENVIRONMENT_DEVICE.to_owned());
        if let Err(error) = thread::Builder::new()
            .name("particulates".into())
            .spawn(|| loop {
                if let Some(reading) = fetch_particulates() {
                    for (name, value) in [("pm10", reading.pm10), ("pm25", reading.pm25)] {
                        if let Err(error) =
                            write_sensor(Path::new(SENSOR_DIR), name, &value.to_string())
                        {
                            log::error!("Cannot save {}: {}", name, error);
                        }
                    }
                }
                thread::sleep(Duration::from_secs(10));
            })
        {
            log::error!("Cannot start particulate reader: {}", error);
        }
    });
}

fn first_particulate_reading<T>(
    ports: Vec<PathBuf>,
    mut read: impl FnMut(&Path) -> Option<T>,
) -> Option<T> {
    for path in ports {
        let Some(_claim) = PortClaim::acquire(&path) else {
            continue;
        };
        // Advance on every failure, including setup and query failures.
        if let Some(reading) = read(&path) {
            return Some(reading);
        }
    }
    None
}

fn fetch_particulates() -> Option<Message> {
    first_particulate_reading(sensor_ports_in(Path::new("/dev")), |path| {
        let mut sensor = SDS011::new(path.to_str()?).ok()?;
        sensor.set_work_period(10).ok()?;
        sensor.query().ok()
    })
}

pub fn fetch_pm25() -> String {
    fetch_particulates()
        .map(|reading| reading.pm25.to_string())
        .unwrap_or_default()
}

pub fn fetch_pm10() -> String {
    fetch_particulates()
        .map(|reading| reading.pm10.to_string())
        .unwrap_or_default()
}

fn field<'a>(raw: &'a str, key: &str) -> Option<&'a str> {
    let key = key.trim().trim_end_matches(':');
    raw.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.trim() == key).then(|| value.trim())
    })
}

pub fn parse_arduino(raw: String, line_key: &str, on_fail_string: String) -> String {
    field(&raw, line_key)
        .map(str::to_owned)
        .unwrap_or(on_fail_string)
}

pub fn get_value(sensor: &str) -> String {
    fs::read_to_string(Path::new(SENSOR_DIR).join(sensor)).unwrap_or_else(|_| "N/A".into())
}

pub fn overflow_allows_pumping() -> bool {
    overflow_allows_pumping_in(Path::new(SENSOR_DIR))
}

pub(crate) fn overflow_allows_pumping_in(directory: &Path) -> bool {
    !directory.join("overflow_error").exists()
        && ["t1_ovf", "t2_ovf"].iter().all(|name| {
            let path = directory.join(name);
            let fresh = fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age < STALE_TIMEOUT);
            fresh
                && fs::read_to_string(path)
                    .is_ok_and(|value| matches!(value.trim(), "NONE" | "NORMAL"))
        })
}

fn write_sensor(directory: &Path, name: &str, value: &str) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let temporary = directory.join(format!(".{}.tmp", name));
    fs::write(&temporary, value)?;
    // Readers always see a complete value, never a truncated file mid-write.
    fs::rename(temporary, directory.join(name))
}

pub(super) fn set_overflow_failure(directory: &Path, reason: &str) -> io::Result<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Publish the interlock before changing the individual readings.
    write_sensor(
        directory,
        "overflow_error",
        &format!("SENSOR_FAILURE: {} - timestamp: {}", reason, timestamp),
    )?;
    let tank1 = write_sensor(directory, "t1_ovf", "OVERFLOW");
    let tank2 = write_sensor(directory, "t2_ovf", "OVERFLOW");
    tank1.and(tank2)
}

fn report_overflow_failure(directory: &Path, reason: &str) {
    if directory.join("overflow_error").exists() {
        return;
    }
    log::error!(
        "Overflow sensor unavailable: {}; pump operation blocked",
        reason
    );
    if let Err(error) = set_overflow_failure(directory, reason) {
        log::error!("Cannot save overflow safety state: {}", error);
    }
}

#[derive(Default)]
struct FrameDecoder {
    line: Vec<u8>,
    frame: String,
    collecting: bool,
    discard_line: bool,
}

impl FrameDecoder {
    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut frames = Vec::new();
        for &byte in bytes {
            if byte != b'\n' {
                if !self.discard_line {
                    self.line.push(byte);
                    if self.line.len() > MAX_LINE_BYTES {
                        self.line.clear();
                        self.frame.clear();
                        self.collecting = false;
                        self.discard_line = true;
                    }
                }
                continue;
            }
            if self.discard_line {
                self.discard_line = false;
                continue;
            }
            match std::str::from_utf8(&self.line).map(str::trim) {
                Ok("BEGIN") => {
                    self.frame.clear();
                    self.collecting = true;
                }
                Ok("END") if self.collecting => {
                    frames.push(std::mem::take(&mut self.frame));
                    self.collecting = false;
                }
                Ok(line) if self.collecting && self.frame.len() + line.len() < MAX_FRAME_BYTES => {
                    self.frame.push_str(line);
                    self.frame.push('\n');
                }
                _ => {
                    self.frame.clear();
                    self.collecting = false;
                }
            }
            self.line.clear();
        }
        frames
    }
}

fn valid_number<'a>(frame: &'a str, key: &str, unit: &str, min: f64, max: f64) -> Option<&'a str> {
    let text = field(frame, key)?;
    let number = text
        .strip_suffix(unit)
        .unwrap_or(text)
        .trim()
        .parse::<f64>()
        .ok()?;
    (number.is_finite() && (min..=max).contains(&number)).then_some(text)
}

fn save_frame(directory: &Path, device: &str, frame: &str) -> io::Result<bool> {
    if device == OVERFLOW_DEVICE {
        let tank1 = field(frame, "T1_OVF");
        let tank2 = field(frame, "T2_OVF");
        if !matches!(tank1, Some("NONE" | "OVERFLOW"))
            || !matches!(tank2, Some("NONE" | "OVERFLOW"))
        {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "invalid or missing overflow readings",
            ));
        }
        write_sensor(directory, "t1_ovf", tank1.unwrap())?;
        write_sensor(directory, "t2_ovf", tank2.unwrap())?;
        // Only a complete, validated report from the correct device can recover.
        match fs::remove_file(directory.join("overflow_error")) {
            Ok(()) => log::info!("Overflow sensor communication recovered"),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if let Some(ph) = valid_number(frame, "PH", "", 0.0, 14.0) {
            write_sensor(directory, "ph", ph)?;
        }
        Ok(true)
    } else {
        let mut saved = false;
        for (key, name, unit, min, max) in [
            ("CO2", "co2", "ppm", 0.0, 1_000_000.0),
            ("TVOC", "tvoc", "ppb", 0.0, 100_000.0),
            ("TEMP", "temp", "C", -80.0, 85.0),
            ("HUM", "hum", "%", 0.0, 100.0),
        ] {
            if let Some(value) = valid_number(frame, key, unit, min, max) {
                write_sensor(directory, name, value)?;
                saved = true;
            }
        }
        Ok(saved)
    }
}

struct ArduinoReader<'a> {
    device: &'a str,
    directory: &'a Path,
    decoder: FrameDecoder,
    matched: bool,
    last_valid: Instant,
}

impl<'a> ArduinoReader<'a> {
    fn new(device: &'a str, directory: &'a Path, now: Instant) -> Self {
        Self {
            device,
            directory,
            decoder: FrameDecoder::default(),
            matched: false,
            last_valid: now,
        }
    }

    fn poll(&mut self, read: io::Result<&[u8]>, now: Instant) -> io::Result<bool> {
        let result = self.poll_inner(read, now);
        if let Err(error) = &result {
            if self.matched && self.device == OVERFLOW_DEVICE {
                report_overflow_failure(self.directory, &error.to_string());
            }
        }
        result
    }

    fn poll_inner(&mut self, read: io::Result<&[u8]>, now: Instant) -> io::Result<bool> {
        match read {
            Ok([]) => {
                return Err(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "serial device disconnected",
                ))
            }
            Ok(bytes) => {
                for frame in self.decoder.push(bytes) {
                    match field(&frame, "DEVICE_ID") {
                        Some(device) if device == self.device => {
                            self.matched = true;
                            if save_frame(self.directory, device, &frame)? {
                                self.last_valid = now;
                            }
                        }
                        Some(_) if self.matched => {
                            return Err(io::Error::new(
                                ErrorKind::InvalidData,
                                "serial device identity changed",
                            ));
                        }
                        Some(_) => return Ok(false), // Release the port for the correct reader.
                        None => {}
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error),
        }
        let timeout = if self.matched {
            STALE_TIMEOUT
        } else {
            DISCOVERY_TIMEOUT
        };
        if now.duration_since(self.last_valid) >= timeout {
            return Err(io::Error::new(
                ErrorKind::TimedOut,
                "no valid sensor report before deadline",
            ));
        }
        Ok(true)
    }
}

fn monitor_arduino_port(path: &Path, device: &str, directory: &Path) {
    for baud_rate in ARDUINO_BAUD_RATES {
        let mut port = match SerialPort::open(path, baud_rate) {
            Ok(port) => port,
            Err(error) => {
                log::debug!("Cannot open {}: {}", path.display(), error);
                return;
            }
        };
        if let Err(error) = port.set_read_timeout(Duration::from_millis(100)) {
            log::warn!("Cannot configure {}: {}", path.display(), error);
            return;
        }
        let mut reader = ArduinoReader::new(device, directory, Instant::now());
        let mut buffer = [0; 256];
        loop {
            let read = port.read(&mut buffer).map(|count| &buffer[..count]);
            let retry = read.as_ref().is_err_and(|error| {
                matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted)
            });
            let was_matched = reader.matched;
            match reader.poll(read, Instant::now()) {
                Ok(true) => {
                    if reader.matched && !was_matched {
                        log::info!(
                            "Found {} at {} ({} baud)",
                            device,
                            path.display(),
                            baud_rate
                        );
                    }
                }
                Ok(false) => return,
                Err(error) => {
                    if reader.matched {
                        log::warn!("{} at {}: {}", device, path.display(), error);
                        return;
                    }
                    break;
                }
            }
            if retry {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

pub fn fetch_arduino(device_type: String) {
    static OVERFLOW_STARTED: Once = Once::new();
    static ENVIRONMENT_STARTED: Once = Once::new();
    let started = match device_type.as_str() {
        OVERFLOW_DEVICE => &OVERFLOW_STARTED,
        ENVIRONMENT_DEVICE => &ENVIRONMENT_STARTED,
        _ => {
            log::error!("Unsupported Arduino device: {}", device_type);
            return;
        }
    };
    started.call_once(|| {
        if device_type == OVERFLOW_DEVICE {
            // Old values from a previous process must not authorize pump operation.
            if let Err(error) =
                set_overflow_failure(Path::new(SENSOR_DIR), "waiting for overflow sensor")
            {
                log::error!("Cannot initialize overflow safety state: {}", error);
            }
        }
        if let Err(error) = thread::Builder::new()
            .name(device_type.clone())
            .spawn(move || loop {
                for path in sensor_ports_in(Path::new("/dev")) {
                    let Some(_claim) = PortClaim::acquire(&path) else {
                        continue;
                    };
                    monitor_arduino_port(&path, &device_type, Path::new(SENSOR_DIR));
                }
                // Bound retries and CPU use even when no serial devices are attached.
                thread::sleep(Duration::from_secs(1));
            })
        {
            log::error!("Cannot start Arduino reader: {}", error);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};
    use tempfile::TempDir;

    #[test]
    fn test_parse_arduino_valid_input() {
        let raw = "BEGIN\nDEVICE_ID: SENSORKIT_MK1\nCO2: 450ppm\nTEMP: 25C\nEND".to_string();

        let co2 = parse_arduino(raw.clone(), "CO2", "N/A".to_string());
        assert_eq!(co2, "450ppm");

        let temp = parse_arduino(raw.clone(), "TEMP", "N/A".to_string());
        assert_eq!(temp, "25C");

        let missing = parse_arduino(raw.clone(), "HUMIDITY", "N/A".to_string());
        assert_eq!(missing, "N/A");
    }

    #[test]
    fn test_parse_arduino_malformed_input() {
        let raw = "BROKEN DATA WITHOUT COLONS".to_string();
        let result = parse_arduino(raw, "CO2", "ERROR".to_string());
        assert_eq!(result, "ERROR");
    }

    #[test]
    fn test_parse_arduino_empty_input() {
        let raw = "".to_string();
        let result = parse_arduino(raw, "CO2", "EMPTY".to_string());
        assert_eq!(result, "EMPTY");
    }

    #[test]
    fn test_parse_arduino_multiple_values() {
        let raw = "CO2: 400ppm\nCO2: 500ppm\nCO2: 600ppm".to_string();
        let result = parse_arduino(raw, "CO2", "N/A".to_string());
        assert_eq!(result, "400ppm");
    }

    #[test]
    fn test_get_value_existing_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let sensors_dir = temp_dir.path().join("sensors");
        fs::create_dir_all(&sensors_dir).expect("Failed to create sensors directory for test");

        let test_file = sensors_dir.join("test_sensor");
        let mut file = fs::File::create(&test_file).expect("Failed to create test sensor file");
        file.write_all(b"42.5").expect("Failed to write test data");

        std::env::set_var(
            "TEST_SENSOR_PATH",
            test_file
                .to_str()
                .expect("Failed to convert path to string"),
        );

        let mut data = String::new();
        if let Ok(mut f) = fs::File::open(test_file) {
            let _ = f.read_to_string(&mut data);
        }
        assert_eq!(data, "42.5");
    }

    #[test]
    fn test_get_value_missing_file() {
        let result = get_value("non_existent_sensor");
        assert_eq!(result, "N/A");
    }

    #[test]
    fn test_parse_arduino_with_special_characters() {
        let raw = "SENSOR: value!@#$%^&*()\nTEMP: 25.5°C".to_string();
        let sensor = parse_arduino(raw.clone(), "SENSOR", "N/A".to_string());
        assert_eq!(sensor, "value!@#$%^&*()");
        let temp = parse_arduino(raw, "TEMP", "N/A".to_string());
        assert_eq!(temp, "25.5°C");
    }

    #[test]
    fn test_parse_arduino_edge_cases() {
        let raw = "KEY: \nEMPTY_VALUE: ".to_string();
        let key_result = parse_arduino(raw.clone(), "KEY", "DEFAULT".to_string());
        assert_eq!(key_result, "");
        let empty_result = parse_arduino(raw, "EMPTY_VALUE", "DEFAULT".to_string());
        assert_eq!(empty_result, "");
    }
}

#[cfg(test)]
mod serial_tests {
    use super::*;
    use tempfile::TempDir;

    const HEALTHY: &[u8] = b"BEGIN\r\nDEVICE_ID: DUAL_OVF_SENSOR\r\nT1_OVF: NONE\r\nT2_OVF: NONE\r\nPH: 7.1\r\nEND\r\n";

    #[test]
    fn legacy_kit_can_boot_before_the_normal_staleness_deadline_applies() {
        let directory = TempDir::new().unwrap();
        set_overflow_failure(directory.path(), "startup").unwrap();
        let now = Instant::now();
        let mut reader = ArduinoReader::new(OVERFLOW_DEVICE, directory.path(), now);
        reader
            .poll(
                Ok(b"\xffBEGIN\npartial boot report\nEND\n"),
                now + Duration::from_secs(3),
            )
            .unwrap();
        reader
            .poll(
                Err(ErrorKind::TimedOut.into()),
                now + Duration::from_millis(5200),
            )
            .unwrap();
        assert!(!overflow_allows_pumping_in(directory.path()));
        reader
            .poll(Ok(HEALTHY), now + Duration::from_secs(6))
            .unwrap();
        assert!(overflow_allows_pumping_in(directory.path()));
        assert!(reader
            .poll(
                Err(ErrorKind::TimedOut.into()),
                now + Duration::from_secs(6) + STALE_TIMEOUT
            )
            .is_err());
        assert!(!overflow_allows_pumping_in(directory.path()));
    }

    #[test]
    fn fragmented_frames_recover_only_after_the_end_marker() {
        let directory = TempDir::new().unwrap();
        set_overflow_failure(directory.path(), "startup").unwrap();
        let now = Instant::now();
        let mut reader = ArduinoReader::new(OVERFLOW_DEVICE, directory.path(), now);
        for byte in &HEALTHY[..HEALTHY.len() - 1] {
            reader.poll(Ok(std::slice::from_ref(byte)), now).unwrap();
            assert!(!overflow_allows_pumping_in(directory.path()));
        }
        reader.poll(Ok(b"\n"), now).unwrap();
        assert!(overflow_allows_pumping_in(directory.path()));
        assert_eq!(
            fs::read_to_string(directory.path().join("ph")).unwrap(),
            "7.1"
        );
    }

    #[test]
    fn temporary_errors_preserve_readings_but_sustained_silence_blocks_pumps() {
        let directory = TempDir::new().unwrap();
        let now = Instant::now();
        let mut reader = ArduinoReader::new(OVERFLOW_DEVICE, directory.path(), now);
        reader.poll(Ok(HEALTHY), now).unwrap();
        for (i, kind) in [
            ErrorKind::WouldBlock,
            ErrorKind::TimedOut,
            ErrorKind::Interrupted,
        ]
        .into_iter()
        .enumerate()
        {
            assert!(reader
                .poll(Err(kind.into()), now + Duration::from_secs(i as u64 + 1))
                .unwrap());
            assert!(overflow_allows_pumping_in(directory.path()));
        }
        let error = reader
            .poll(Err(ErrorKind::WouldBlock.into()), now + STALE_TIMEOUT)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TimedOut);
        assert!(!overflow_allows_pumping_in(directory.path()));
        assert_eq!(
            fs::read_to_string(directory.path().join("t2_ovf")).unwrap(),
            "OVERFLOW"
        );
        reader.poll(Ok(HEALTHY), now + STALE_TIMEOUT).unwrap();
        assert!(overflow_allows_pumping_in(directory.path()));
    }

    #[test]
    fn noise_and_partial_frames_cannot_keep_stale_readings_alive() {
        let directory = TempDir::new().unwrap();
        let now = Instant::now();
        let mut reader = ArduinoReader::new(OVERFLOW_DEVICE, directory.path(), now);
        reader.poll(Ok(HEALTHY), now).unwrap();
        reader
            .poll(
                Ok(b"\xff\0\nBEGIN\nT1_OVF: NONE\n"),
                now + Duration::from_secs(4),
            )
            .unwrap();
        assert!(reader
            .poll(Ok(b"T2_OVF: NONE"), now + STALE_TIMEOUT)
            .is_err());
        assert!(!overflow_allows_pumping_in(directory.path()));
    }

    #[test]
    fn unrelated_devices_do_not_clear_the_overflow_interlock() {
        let directory = TempDir::new().unwrap();
        set_overflow_failure(directory.path(), "startup").unwrap();
        let now = Instant::now();
        let mut reader = ArduinoReader::new(OVERFLOW_DEVICE, directory.path(), now);
        assert!(!reader
            .poll(
                Ok(b"BEGIN\nDEVICE_ID: SENSORKIT_MK1\nCO2: 400ppm\nEND\n"),
                now
            )
            .unwrap());
        assert!(!reader.matched);
        assert!(!overflow_allows_pumping_in(directory.path()));
    }

    #[test]
    fn malformed_overflow_reports_and_disconnects_fail_closed() {
        for invalid in [
            b"BEGIN\nDEVICE_ID: DUAL_OVF_SENSOR\nT1_OVF: NONE\nEND\n".as_slice(),
            b"BEGIN\nDEVICE_ID: DUAL_OVF_SENSOR\nT1_OVF: NONE\nT2_OVF: N/A\nEND\n",
            b"BEGIN\nDEVICE_ID: SENSORKIT_MK1\nEND\n",
            b"",
        ] {
            let directory = TempDir::new().unwrap();
            let now = Instant::now();
            let mut reader = ArduinoReader::new(OVERFLOW_DEVICE, directory.path(), now);
            reader.poll(Ok(HEALTHY), now).unwrap();
            assert!(reader.poll(Ok(invalid), now).is_err());
            assert!(!overflow_allows_pumping_in(directory.path()));
        }
        for kind in [
            ErrorKind::BrokenPipe,
            ErrorKind::NotConnected,
            ErrorKind::Other,
        ] {
            let directory = TempDir::new().unwrap();
            let now = Instant::now();
            let mut reader = ArduinoReader::new(OVERFLOW_DEVICE, directory.path(), now);
            reader.poll(Ok(HEALTHY), now).unwrap();
            assert_eq!(reader.poll(Err(kind.into()), now).unwrap_err().kind(), kind);
            assert!(!overflow_allows_pumping_in(directory.path()));
        }
    }

    #[test]
    fn multiple_frames_preserve_actual_overflow_and_recover_communication() {
        let directory = TempDir::new().unwrap();
        set_overflow_failure(directory.path(), "startup").unwrap();
        let now = Instant::now();
        let mut reader = ArduinoReader::new(OVERFLOW_DEVICE, directory.path(), now);
        let mut frames = HEALTHY.to_vec();
        frames.extend_from_slice(
            b"BEGIN\nDEVICE_ID: DUAL_OVF_SENSOR\nT1_OVF: OVERFLOW\nT2_OVF: NONE\nEND\n",
        );
        reader.poll(Ok(&frames), now).unwrap();
        assert!(!directory.path().join("overflow_error").exists());
        assert!(!overflow_allows_pumping_in(directory.path()));
        assert_eq!(
            fs::read_to_string(directory.path().join("t1_ovf")).unwrap(),
            "OVERFLOW"
        );
    }

    #[test]
    fn decoder_bounds_noise_and_resynchronizes_at_begin() {
        let mut decoder = FrameDecoder::default();
        assert!(decoder.push(&vec![b'x'; MAX_LINE_BYTES * 100]).is_empty());
        assert!(decoder.line.len() <= MAX_LINE_BYTES);
        decoder.push(b"\nBEGIN\nold partial data\n");
        assert_eq!(decoder.push(HEALTHY).len(), 1);
        let oversized = format!("BEGIN\n{}END\n", "x: 1\n".repeat(MAX_FRAME_BYTES));
        assert!(decoder.push(oversized.as_bytes()).is_empty());
        assert_eq!(decoder.push(HEALTHY).len(), 1);
    }

    #[test]
    fn invalid_environment_values_are_not_published() {
        let directory = TempDir::new().unwrap();
        let now = Instant::now();
        let mut reader = ArduinoReader::new(ENVIRONMENT_DEVICE, directory.path(), now);
        reader
            .poll(
                Ok(b"BEGIN\nDEVICE_ID: SENSORKIT_MK1\nCO2: 450ppm\nHUM: 52%\nTEMP: -5C\nEND\n"),
                now,
            )
            .unwrap();
        reader.poll(Ok(b"BEGIN\nDEVICE_ID: SENSORKIT_MK1\nCO2: NaNppm\nTVOC: 0ppb\nHUM: -999%\nTEMP: -999C\nEND\n"), now).unwrap();
        for (name, value) in [
            ("co2", "450ppm"),
            ("hum", "52%"),
            ("temp", "-5C"),
            ("tvoc", "0ppb"),
        ] {
            assert_eq!(
                fs::read_to_string(directory.path().join(name)).unwrap(),
                value
            );
        }
    }

    #[test]
    fn field_matching_is_exact_and_supports_crlf() {
        let raw = "S1CO2: 900ppm\r\nCO2: 400ppm\r\n";
        assert_eq!(parse_arduino(raw.into(), "CO2:", "N/A".into()), "400ppm");
    }

    #[test]
    fn port_claims_exclude_all_other_readers_until_the_owner_drops() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("ttyUSB0");
        fs::write(&path, "").unwrap();
        let claim = PortClaim::acquire(&path).unwrap();
        assert!(PortClaim::acquire(&path).is_none());
        assert!(PortClaim::acquire(&path).is_none());
        let mut probed = false;
        assert!(first_particulate_reading(vec![path.clone()], |_| {
            probed = true;
            Some(1)
        })
        .is_none());
        assert!(!probed);
        drop(claim);
        assert!(PortClaim::acquire(&path).is_some());
    }

    #[test]
    fn particulate_probe_advances_after_setup_or_query_failure() {
        let directory = TempDir::new().unwrap();
        let ports: Vec<_> = (0..3)
            .map(|i| directory.path().join(format!("ttyUSB{}", i)))
            .collect();
        let mut attempted = Vec::new();
        let result = first_particulate_reading(ports.clone(), |path| {
            attempted.push(path.to_path_buf());
            (path == ports[2]).then_some(42)
        });
        assert_eq!(attempted, ports);
        assert_eq!(result, Some(42));
    }

    #[test]
    fn discovery_includes_acm_and_high_numbered_usb_ports() {
        let directory = TempDir::new().unwrap();
        for name in [
            "ttyUSB0",
            "ttyUSB12",
            "ttyACM0",
            "ttyS0",
            "ttyUSB",
            "ttyUSBbad",
        ] {
            fs::write(directory.path().join(name), "").unwrap();
        }
        let names: Vec<_> = sensor_ports_in(directory.path())
            .into_iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["ttyACM0", "ttyUSB0", "ttyUSB12"]);
    }

    #[test]
    fn missing_empty_invalid_or_faulted_sensor_files_block_pumping() {
        let directory = TempDir::new().unwrap();
        assert!(!overflow_allows_pumping_in(directory.path()));
        fs::write(directory.path().join("t1_ovf"), "NONE").unwrap();
        for value in ["", "N/A", "garbage", "OVERFLOW"] {
            fs::write(directory.path().join("t2_ovf"), value).unwrap();
            assert!(!overflow_allows_pumping_in(directory.path()));
        }
        fs::write(directory.path().join("t2_ovf"), "NONE").unwrap();
        assert!(overflow_allows_pumping_in(directory.path()));
        fs::write(directory.path().join("overflow_error"), "failure").unwrap();
        assert!(!overflow_allows_pumping_in(directory.path()));
    }

    #[test]
    fn stale_normal_readings_block_pumping_even_without_an_error_file() {
        let directory = TempDir::new().unwrap();
        for tank in ["t1_ovf", "t2_ovf"] {
            fs::write(directory.path().join(tank), "NONE").unwrap();
        }
        let file = fs::File::options()
            .write(true)
            .open(directory.path().join("t1_ovf"))
            .unwrap();
        let stale = SystemTime::now() - STALE_TIMEOUT - Duration::from_secs(1);
        file.set_times(fs::FileTimes::new().set_modified(stale))
            .unwrap();
        assert!(!overflow_allows_pumping_in(directory.path()));
    }
}
