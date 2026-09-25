//! Arduino protocol v2 diagnostics and acknowledged, durable replay storage.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const DEVICES: [&str; 2] = ["SENSORKIT_MK1", "DUAL_OVF_SENSOR"];
const RETENTION: i64 = 7 * 86400;
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
pub fn fields(frame: &str) -> BTreeMap<String, String> {
    frame
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect()
}
fn number(fields: &BTreeMap<String, String>, key: &str) -> Option<i64> {
    fields.get(key)?.parse().ok()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub device_id: String,
    pub port: String,
    pub observed_at: i64,
    pub fields: BTreeMap<String, String>,
    pub error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub device_id: String,
    pub boot_id: String,
    pub sequence: u32,
    pub timestamp: i64,
    pub received_at: i64,
    pub time_quality: String,
    pub fields: BTreeMap<String, String>,
    #[serde(default)]
    pub co2: Option<super::co2::Reading>,
}
struct StoreState {
    records: Vec<DeviceRecord>,
    identities: HashSet<(String, String, u32)>,
    error: Option<String>,
}
struct Store {
    state: Mutex<StoreState>,
    writing: Mutex<()>,
}
lazy_static::lazy_static! {
    static ref STORES: Mutex<HashMap<PathBuf, Arc<Store>>> = Mutex::new(HashMap::new());
}
fn store(directory: &Path) -> Arc<Store> {
    let mut stores = STORES.lock().unwrap_or_else(|e| e.into_inner());
    stores
        .entry(directory.to_owned())
        .or_insert_with(|| {
            let mut state = StoreState {
                records: Vec::new(),
                identities: HashSet::new(),
                error: None,
            };
            let logs = directory.join("device_logs/v2");
            if logs.exists() {
                match fs::read_dir(logs) {
                    Ok(entries) => {
                        for entry in entries.filter_map(Result::ok) {
                            if entry.path().extension().and_then(|s| s.to_str()) != Some("jsonl") {
                                continue;
                            }
                            match File::open(entry.path()) {
                                Ok(file) => {
                                    for line in BufReader::new(file).split(b'\n') {
                                        let Ok(line) = line else {
                                            state.error = Some("Cannot read device log".into());
                                            break;
                                        };
                                        if let Ok(record) =
                                            serde_json::from_slice::<DeviceRecord>(&line)
                                        {
                                            if (now() - RETENTION..=now() + 5)
                                                .contains(&record.timestamp)
                                                && DEVICES.contains(&record.device_id.as_str())
                                                && state.identities.insert((
                                                    record.device_id.clone(),
                                                    record.boot_id.clone(),
                                                    record.sequence,
                                                ))
                                            {
                                                state.records.push(record);
                                            }
                                        }
                                    }
                                }
                                Err(e) => state.error = Some(e.to_string()),
                            }
                        }
                    }
                    Err(e) => state.error = Some(e.to_string()),
                }
            }
            Arc::new(Store {
                state: Mutex::new(state),
                writing: Mutex::new(()),
            })
        })
        .clone()
}

/// A successful return is the ONLY condition that permits AOG_ACK. Every
/// record contains the replay identity so retrying after a Pi crash is safe.
pub fn absorb(
    directory: &Path,
    device: &str,
    raw: &BTreeMap<String, String>,
) -> io::Result<String> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid replay identity or timestamp",
        )
    };
    let boot = raw
        .get("BOOT_ID")
        .filter(|id| {
            id.len() == 16
                && id.bytes().all(|b| b.is_ascii_hexdigit())
                && id.as_str() != "0000000000000000"
        })
        .ok_or_else(invalid)?;
    let sequence = raw
        .get("SEQ")
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|s| *s > 0)
        .ok_or_else(invalid)?;
    let uptime = number(raw, "UPTIME_S").ok_or_else(invalid)?;
    let sample_uptime = number(raw, "SAMPLE_UPTIME_S").ok_or_else(invalid)?;
    if !DEVICES.contains(&device)
        || sample_uptime < 0
        || uptime < sample_uptime
        || uptime - sample_uptime > RETENTION
    {
        return Err(invalid());
    }
    let reported = number(raw, "TIMESTAMP").unwrap_or(0);
    let (timestamp, quality) = if reported > 0 {
        (reported, "synchronized")
    } else {
        (now() - (uptime - sample_uptime), "estimated_from_uptime")
    };
    if !(now() - RETENTION..=now() + 5).contains(&timestamp) {
        return Err(invalid());
    }
    let shared = store(directory);
    let _writing = shared.writing.lock().unwrap_or_else(|e| e.into_inner());
    let state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
    let identity = (device.to_owned(), boot.to_owned(), sequence);
    let ack = format!("AOG_ACK {} {}\n", boot, sequence);
    // If an existing log cannot be read, do not acknowledge and silently hide it.
    if let Some(error) = &state.error {
        return Err(io::Error::other(error.clone()));
    }
    if state.identities.contains(&identity) {
        let saved = state
            .records
            .iter()
            .find(|r| r.device_id == device && r.boot_id == *boot && r.sequence == sequence)
            .ok_or_else(invalid)?;
        let day = chrono::DateTime::from_timestamp(saved.timestamp, 0)
            .ok_or_else(invalid)?
            .format("%Y-%m-%d")
            .to_string();
        drop(state);
        // A process could have crashed after write() but before fsync(). A
        // valid row recovered from the page cache still needs syncing before ACK.
        let logs = directory.join("device_logs/v2");
        File::open(logs.join(format!("{day}.jsonl")))?.sync_all()?;
        File::open(&logs)?.sync_all()?;
        File::open(directory.join("device_logs"))?.sync_all()?;
        File::open(directory)?.sync_all()?;
        return Ok(ack);
    }
    drop(state);
    let record = DeviceRecord {
        device_id: device.into(),
        boot_id: boot.clone(),
        sequence,
        timestamp,
        received_at: now(),
        time_quality: quality.into(),
        fields: raw.clone(),
        co2: (device == DEVICES[0]).then(|| super::co2::reading(raw, &super::co2::settings(directory), timestamp)),
    };
    let logs = directory.join("device_logs/v2");
    fs::create_dir_all(&logs)?;
    // Sync each newly created directory entry, including first installation.
    for dir in [&logs, &directory.join("device_logs"), &directory.to_owned()] {
        File::open(dir)?.sync_all()?;
    }
    let day = chrono::DateTime::from_timestamp(timestamp, 0)
        .ok_or_else(invalid)?
        .format("%Y-%m-%d");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join(format!("{day}.jsonl")))?;
    let before = file.metadata()?.len();
    let bytes = format!("\n{}\n", serde_json::to_string(&record)?);
    if let Err(error) = file
        .write_all(bytes.as_bytes())
        .and_then(|_| file.sync_data())
        .and_then(|_| File::open(&logs)?.sync_all())
    {
        file.set_len(before)?;
        file.sync_data()?;
        return Err(error);
    }
    let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
    state.identities.insert(identity);
    state.records.push(record);
    state.records.retain(|r| r.timestamp >= now() - RETENTION);
    if state.identities.len() > 25000 {
        state.identities = state
            .records
            .iter()
            .map(|r| (r.device_id.clone(), r.boot_id.clone(), r.sequence))
            .collect();
    }
    drop(state);
    // Prune only dated device journals outside the documented retention window.
    for entry in fs::read_dir(&logs)?.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            if let Some(date) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            {
                if date < (chrono::Utc::now() - chrono::Duration::seconds(RETENTION)).date_naive() {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
    Ok(ack)
}
pub fn records(directory: &Path, start: i64, end: i64) -> Vec<DeviceRecord> {
    store(directory)
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .records
        .iter()
        .filter(|r| (start..=end).contains(&r.timestamp))
        .cloned()
        .collect()
}
pub struct ReplaySample {
    pub device_id: String,
    pub timestamp: i64,
    pub values: [Option<f64>; 8],
    pub ph: Option<f32>,
    pub co2_source: Option<String>,
}
// Charts copy compact numeric samples, not thousands of raw frame strings.
pub fn samples(directory: &Path, start: i64, end: i64) -> Vec<ReplaySample> {
    store(directory)
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .records
        .iter()
        .filter(|r| (start..=end).contains(&r.timestamp))
        .map(|r| {
            let parse = |key: &str, suffix: &str, low: f64, high: f64| {
                r.fields
                    .get(key)
                    .and_then(|s| s.trim_end_matches(suffix).trim().parse::<f64>().ok())
                    .filter(|n| n.is_finite() && (low..=high).contains(n))
            };
            let mut values = [None; 8];
            if r.device_id == DEVICES[0] {
                values[0] = r.co2.as_ref().map_or_else(|| parse("CO2", "ppm", 0.0, 1_000_000.0), |v| v.value)
                    .filter(|v| v.is_finite() && (0.0..=1_000_000.0).contains(v));
                values[1] = parse("TVOC", "ppb", 0.0, 100_000.0);
                values[4] = parse("TEMP", "C", -80.0, 85.0);
                values[5] = parse("HUM", "%", 0.0, 100.0);
            }
            values[6] = parse("WATER_D6", "C", -55.0, 125.0);
            values[7] = parse("WATER_D8", "C", -55.0, 125.0);
            ReplaySample {
                device_id: r.device_id.clone(),
                timestamp: r.timestamp,
                values,
                ph: parse("PH", "", 0.0, 14.0).map(|n| n as f32),
                co2_source: (r.device_id == DEVICES[0]).then(|| r.co2.as_ref().map(|v| v.source.clone()).unwrap_or_else(|| {
                    if r.fields.get("PROTOCOL_VERSION").is_some_and(|v| v == "2") { "ccs811" } else { "legacy_unattributed" }.into()
                })),
            }
        })
        .collect()
}
pub fn save_status(directory: &Path, status: &DeviceStatus) -> io::Result<()> {
    if !DEVICES.contains(&status.device_id.as_str()) {
        return Err(io::Error::other("Unknown board"));
    }
    let devices = directory.join("sensors/devices");
    fs::create_dir_all(&devices)?;
    let path = devices.join(format!("{}.json", status.device_id));
    let temp = path.with_extension("tmp");
    fs::write(&temp, serde_json::to_vec(status)?)?;
    fs::rename(temp, path)
}
pub fn statuses(directory: &Path) -> Vec<DeviceStatus> {
    DEVICES
        .iter()
        .map(|device| {
            fs::read(directory.join(format!("sensors/devices/{device}.json")))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<DeviceStatus>(&bytes).ok())
                .unwrap_or(DeviceStatus {
                    device_id: (*device).into(),
                    port: String::new(),
                    observed_at: 0,
                    fields: BTreeMap::new(),
                    error: None,
                })
        })
        .collect()
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterProbe {
    pub label: String,
    pub source: Option<String>,
}
pub fn water_config(directory: &Path) -> [WaterProbe; 2] {
    fs::read(directory.join("water_probes.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| {
            [
                WaterProbe {
                    label: "Water · D6".into(),
                    source: None,
                },
                WaterProbe {
                    label: "Water · D8".into(),
                    source: None,
                },
            ]
        })
}
pub fn save_water_config(directory: &Path, probes: [WaterProbe; 2]) -> io::Result<()> {
    if probes.iter().any(|p| {
        p.label.trim().is_empty()
            || p.label.chars().count() > 40
            || p.label.chars().any(char::is_control)
            || p.source
                .as_ref()
                .is_some_and(|s| !DEVICES.contains(&s.as_str()))
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Use a label of 1–40 characters and a supported Arduino",
        ));
    }
    let path = directory.join("water_probes.json");
    let temp = path.with_extension("tmp");
    let mut file = File::create(&temp)?;
    file.write_all(&serde_json::to_vec(&probes)?)?;
    file.sync_all()?;
    fs::rename(temp, path)?;
    File::open(directory)?.sync_all()
}
pub fn water_reading(directory: &Path, pin: u8) -> Option<(f64, i64)> {
    let config = water_config(directory);
    let source = &config[usize::from(pin == 8)].source;
    let key = format!("WATER_D{pin}");
    let values: Vec<_> = statuses(directory)
        .into_iter()
        .filter(|s| {
            (0..=15).contains(&(now() - s.observed_at))
                && !s
                    .error
                    .as_ref()
                    .is_some_and(|e| e.starts_with("Live report failed:"))
                && source.as_ref().is_none_or(|id| id == &s.device_id)
        })
        .filter(|s| {
            s.fields
                .get(&format!("{key}_STATUS"))
                .is_some_and(|status| status == "ok")
        })
        .filter_map(|s| parse_temperature(s.fields.get(&key)?).map(|v| (v, s.observed_at)))
        .collect();
    // Never silently mix two boards on the same D pin.
    (values.len() == 1).then(|| values[0])
}
pub fn parse_temperature(raw: &str) -> Option<f64> {
    raw.trim()
        .trim_end_matches('C')
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && (-55.0..=125.0).contains(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn replay() -> BTreeMap<String, String> {
        fields("BOOT_ID: 1234567890abcdef\nSEQ: 1\nUPTIME_S: 180\nSAMPLE_UPTIME_S: 60\nTIMESTAMP: 0\nWATER_D6: 22.5C")
    }
    #[test]
    fn durable_ack_and_duplicate_replay() {
        let dir = tempfile::tempdir().unwrap();
        let ack = absorb(dir.path(), DEVICES[0], &replay()).unwrap();
        assert_eq!(ack, "AOG_ACK 1234567890abcdef 1\n");
        // Recreate the store to simulate a controller restart after saving but before ACK delivery.
        STORES.lock().unwrap().remove(dir.path());
        assert_eq!(absorb(dir.path(), DEVICES[0], &replay()).unwrap(), ack);
        let rows = records(dir.path(), now() - 180, now());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].time_quality, "estimated_from_uptime");
        assert!((now() - rows[0].timestamp - 120).abs() < 2);
    }
    #[test]
    fn recovered_history_uses_reported_co2_without_averaging_adc_counts() {
        let dir = tempfile::tempdir().unwrap(); let mut frame = replay();
        frame.insert("CO2".into(), "1200ppm".into());
        frame.insert("CO2_ADC".into(), "160".into());
        absorb(dir.path(), DEVICES[0], &frame).unwrap();
        let rows = samples(dir.path(), now() - 180, now());
        assert_eq!(rows.len(), 1); assert_eq!(rows[0].values[0], Some(1200.0));
        frame.insert("SEQ".into(), "2".into()); frame.remove("CO2");
        absorb(dir.path(), DEVICES[0], &frame).unwrap();
        let rows = samples(dir.path(), now() - 180, now());
        assert_eq!(rows[1].values[0], None);
    }
    #[test]
    fn recovered_infrared_co2_keeps_its_conversion_after_settings_change() {
        let dir = tempfile::tempdir().unwrap(); let mut frame = replay();
        let config = super::super::co2::Settings { analog_model: super::super::co2::ANALOG_SOURCE.into(), reference_mv: 5000, primary_source: "analog".into() };
        super::super::co2::save_settings(dir.path(), &config).unwrap();
        frame.insert("CO2".into(), "400ppm".into()); frame.insert("CO2_ADC".into(), "160".into());
        absorb(dir.path(), DEVICES[0], &frame).unwrap();
        super::super::co2::save_settings(dir.path(), &super::super::co2::Settings::default()).unwrap();
        STORES.lock().unwrap().remove(dir.path());
        absorb(dir.path(), DEVICES[0], &frame).unwrap(); // duplicate must not be reinterpreted
        let rows = samples(dir.path(), now()-180, now());
        assert_eq!(rows.len(), 1); assert_eq!(rows[0].values[0], Some(1191.40625));
        assert_eq!(rows[0].co2_source.as_deref(), Some(super::super::co2::ANALOG_SOURCE));
    }
    #[test]
    fn failed_storage_never_acknowledges() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("device_logs"), "not a directory").unwrap();
        assert!(absorb(dir.path(), DEVICES[1], &replay()).is_err());
        assert!(records(dir.path(), 0, now()).is_empty());
    }
    #[test]
    fn rejects_invalid_or_future_replay() {
        let dir = tempfile::tempdir().unwrap();
        let mut frame = replay();
        frame.insert("TIMESTAMP".into(), (now() + 3600).to_string());
        assert!(absorb(dir.path(), DEVICES[0], &frame).is_err());
        frame = replay();
        frame.insert("BOOT_ID".into(), "../../escape".into());
        assert!(absorb(dir.path(), DEVICES[0], &frame).is_err());
    }
    #[test]
    fn temperature_requires_one_fresh_source_or_an_explicit_binding() {
        let dir = tempfile::tempdir().unwrap();
        for (device, value) in [(DEVICES[0], "21.25C"), (DEVICES[1], "28.5C")] {
            save_status(
                dir.path(),
                &DeviceStatus {
                    device_id: device.into(),
                    port: "/dev/ttyUSB0".into(),
                    observed_at: now(),
                    fields: fields(&format!("WATER_D6: {value}\nWATER_D6_STATUS: ok")),
                    error: None,
                },
            )
            .unwrap();
        }
        assert!(water_reading(dir.path(), 6).is_none());
        let mut config = water_config(dir.path());
        config[0].source = Some(DEVICES[1].into());
        save_water_config(dir.path(), config).unwrap();
        assert_eq!(water_reading(dir.path(), 6).unwrap().0, 28.5);
        assert!(water_reading(dir.path(), 8).is_none());
        assert!(parse_temperature("NaN").is_none());
        assert!(parse_temperature("-127C").is_none());
    }
}
