//! Explain the Sensor Kit's separate gas signals without combining unlike units.
use super::device_link::DeviceStatus;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read, Write},
    path::Path,
    time::SystemTime,
};

pub const ANALOG_SOURCE: &str = "dfrobot_sen0219_analog";
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub analog_model: String,
    pub reference_mv: u16,
    pub primary_source: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            analog_model: "unidentified".into(),
            reference_mv: 5000,
            primary_source: "ccs811".into(),
        }
    }
}
impl Settings {
    fn valid(&self) -> bool {
        matches!(self.analog_model.as_str(), "unidentified" | ANALOG_SOURCE)
            && (3000..=5500).contains(&self.reference_mv)
            && matches!(self.primary_source.as_str(), "ccs811" | "analog")
            && (self.primary_source != "analog" || self.analog_model == ANALOG_SOURCE)
    }
    pub fn source(&self) -> &str {
        if self.primary_source == "analog" {
            ANALOG_SOURCE
        } else {
            "ccs811"
        }
    }
}
pub fn settings(directory: &Path) -> Settings {
    fs::read(directory.join("co2_settings.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
        .filter(Settings::valid)
        .unwrap_or_default()
}
fn atomic_json(path: &Path, value: &impl Serialize, durable: bool) -> io::Result<()> {
    let temp = path.with_extension("tmp");
    let mut file = File::create(&temp)?;
    file.write_all(&serde_json::to_vec(value)?)?;
    if durable {
        file.sync_all()?;
    }
    fs::rename(temp, path)?;
    if durable {
        File::open(path.parent().unwrap())?.sync_all()?;
    }
    Ok(())
}
pub fn save_settings(directory: &Path, value: &Settings) -> io::Result<()> {
    if !value.valid() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Choose a supported analog model and source, with a 3.0–5.5 V ADC reference. Analog CO₂ requires the SEN0219 v1 analog profile."));
    }
    atomic_json(&directory.join("co2_settings.json"), value, true)
}
fn adc(fields: &BTreeMap<String, String>) -> Option<u16> {
    fields
        .get("CO2_ADC")?
        .parse::<u16>()
        .ok()
        .filter(|n| *n <= 1023)
}
fn reported(fields: &BTreeMap<String, String>) -> Option<f64> {
    fields
        .get("CO2")?
        .trim_end_matches("ppm")
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && (0.0..=1_000_000.0).contains(n))
}
pub fn analog_ppm(raw: u16, config: &Settings) -> Option<f64> {
    if config.analog_model != ANALOG_SOURCE || raw > 1023 {
        return None;
    }
    let mv = f64::from(raw) * f64::from(config.reference_mv) / 1024.0;
    // Manufacturer's SEN0219 v1 analog transfer curve. Below 0.4 V may be
    // preheating/fault; above 2 V is outside its specified output range.
    (400.0..=2000.0)
        .contains(&mv)
        .then_some((mv - 400.0) * 3.125)
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Reading {
    pub value: Option<f64>,
    pub source: String,
    pub reference_mv: Option<u16>,
    pub observed_at: i64,
}
pub fn reading(fields: &BTreeMap<String, String>, config: &Settings, observed_at: i64) -> Reading {
    if config.primary_source == "analog" {
        Reading {
            value: adc(fields).and_then(|raw| analog_ppm(raw, config)),
            source: ANALOG_SOURCE.into(),
            reference_mv: Some(config.reference_mv),
            observed_at,
        }
    } else {
        let v2 = fields.get("PROTOCOL_VERSION").is_some_and(|v| v == "2");
        Reading {
            value: reported(fields),
            source: if v2 { "ccs811" } else { "legacy_unattributed" }.into(),
            reference_mv: None,
            observed_at,
        }
    }
}
/// A0 is converted only for an explicitly identified model. This atomic record
/// binds the primary value to its source/reference; readers never mix two files.
pub fn publish(sensor_directory: &Path, fields: &BTreeMap<String, String>) -> io::Result<Reading> {
    let value = reading(
        fields,
        &settings(sensor_directory.parent().unwrap_or(sensor_directory)),
        super::device_link::now(),
    );
    fs::create_dir_all(sensor_directory)?;
    atomic_json(&sensor_directory.join("co2_reading.json"), &value, false)?;
    Ok(value)
}
pub fn source_matches(source: Option<&str>, selected: &str) -> bool {
    if selected == ANALOG_SOURCE {
        source == Some(ANALOG_SOURCE)
    } else {
        matches!(source, None | Some("ccs811" | "legacy_unattributed"))
    }
}
pub fn current(directory: &Path) -> Option<Reading> {
    let config = settings(directory);
    let path = directory.join("sensors/co2_reading.json");
    if path.exists() {
        let r: Reading = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
        return ((0..=15).contains(&(super::device_link::now() - r.observed_at))
            && source_matches(Some(&r.source), config.source())
            && (r.source != ANALOG_SOURCE || r.reference_mv == Some(config.reference_mv))
            && r.value
                .is_some_and(|v| v.is_finite() && (0.0..=1_000_000.0).contains(&v)))
        .then_some(r);
    }
    if config.primary_source == "analog" {
        return None;
    }
    // Compatibility with previously installed controllers and legacy history.
    let file = File::open(directory.join("sensors/co2")).ok()?;
    let stamp = file.metadata().ok()?.modified().ok()?;
    if SystemTime::now().duration_since(stamp).ok()?.as_secs() > 15 {
        return None;
    }
    let mut raw = String::new();
    file.take(128).read_to_string(&mut raw).ok()?;
    let value = raw
        .trim()
        .trim_end_matches("ppm")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && (0.0..=1_000_000.0).contains(v))?;
    Some(Reading {
        value: Some(value),
        source: "legacy_unattributed".into(),
        reference_mv: None,
        observed_at: stamp.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs() as i64,
    })
}

#[derive(Serialize)]
pub struct Diagnostics {
    pub source: &'static str,
    pub observed_at: Option<i64>,
    pub fresh: bool,
    pub reported_ppm: Option<f64>,
    pub digital_eco2_ppm: Option<f64>,
    pub analog_adc: Option<u16>,
    pub analog_millivolts: Option<f64>,
    pub analog_co2_ppm: Option<f64>,
    pub primary_ppm: Option<f64>,
    pub analog_status: &'static str,
    pub assumed_reference_mv: u16,
    pub compensation: &'static str,
    pub startup_warmup: bool,
}

#[cfg(test)]
pub fn diagnostics(devices: &[DeviceStatus], now: i64) -> Diagnostics {
    diagnostics_configured(devices, now, &Settings::default())
}
pub fn diagnostics_configured(
    devices: &[DeviceStatus],
    now: i64,
    config: &Settings,
) -> Diagnostics {
    let device = devices.iter().find(|d| d.device_id == "SENSORKIT_MK1");
    let fresh = device.is_some_and(|d| {
        (0..=5).contains(&(now - d.observed_at))
            && !d
                .error
                .as_deref()
                .is_some_and(|e| e.starts_with("Live report failed:"))
    });
    let field = |key: &str| device.and_then(|d| d.fields.get(key)).map(String::as_str);
    let v2 = field("PROTOCOL_VERSION") == Some("2");
    let source = if config.primary_source == "analog" {
        ANALOG_SOURCE
    } else if v2 {
        "ccs811"
    } else if device.is_some_and(|d| d.observed_at > 0) {
        "legacy_mixed"
    } else {
        "unknown"
    };
    let reported_ppm = fresh
        .then(|| field("CO2")?.trim_end_matches("ppm").parse::<f64>().ok())
        .flatten()
        .filter(|v| v.is_finite() && (0.0..=1_000_000.0).contains(v));
    let digital_eco2_ppm = (v2 && field("CCS811_STATUS") == Some("ok"))
        .then_some(reported_ppm)
        .flatten();
    let analog_adc = fresh
        .then(|| field("CO2_ADC")?.parse::<u16>().ok())
        .flatten()
        .filter(|v| *v <= 1023);
    let analog_co2_ppm = analog_adc.and_then(|raw| analog_ppm(raw, config));
    let mv = analog_adc.map(|raw| f64::from(raw) * f64::from(config.reference_mv) / 1024.0);
    Diagnostics {
        source,
        observed_at: device.map(|d| d.observed_at).filter(|t| *t > 0),
        fresh,
        reported_ppm,
        digital_eco2_ppm,
        analog_adc,
        // ATmega328P DEFAULT reference is AVcc, not a measured, exact 5 V.
        analog_millivolts: mv,
        analog_co2_ppm,
        primary_ppm: if config.primary_source == "analog" {
            analog_co2_ppm
        } else {
            reported_ppm
        },
        analog_status: match analog_adc {
            None => "unavailable",
            Some(0..=1) => "near_ground",
            Some(1022..=1023) => "near_supply",
            _ if config.analog_model == ANALOG_SOURCE && mv.is_some_and(|v| v < 400.0) => {
                "below_range"
            }
            _ if config.analog_model == ANALOG_SOURCE && mv.is_some_and(|v| v > 2000.0) => {
                "above_range"
            }
            _ if analog_co2_ppm.is_some() => "converted",
            _ => "unidentified",
        },
        assumed_reference_mv: config.reference_mv,
        compensation: if !fresh || !v2 {
            "unknown"
        } else if field("DHT_STATUS") == Some("ok") {
            "live"
        } else {
            "unavailable"
        },
        startup_warmup: fresh
            && v2
            && field("UPTIME_S")
                .and_then(|v| v.parse::<u32>().ok())
                .is_some_and(|v| v < 1200),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn infrared() -> Settings {
        Settings {
            analog_model: ANALOG_SOURCE.into(),
            primary_source: "analog".into(),
            reference_mv: 5000,
        }
    }
    #[test]
    fn identified_dfrobot_converts_voltage_and_rejects_faults_without_digital_fallback() {
        let config = infrared();
        assert_eq!(analog_ppm(160, &config), Some(1191.40625));
        for adc in [0, 1, 81, 410, 1023, 1024] {
            assert_eq!(analog_ppm(adc, &config), None);
        }
        assert_eq!(analog_ppm(160, &Settings::default()), None);
        let frame =
            super::super::device_link::fields("PROTOCOL_VERSION: 2\nCO2: 1400ppm\nCO2_ADC: 160");
        assert_eq!(reading(&frame, &config, 1).value, Some(1191.40625));
        let d = diagnostics_configured(
            &[kit(
                "PROTOCOL_VERSION: 2\nCO2: 1400ppm\nCCS811_STATUS: ok\nCO2_ADC: 160",
            )],
            10001,
            &config,
        );
        assert_eq!(d.primary_ppm, Some(1191.40625));
        assert_eq!(d.digital_eco2_ppm, Some(1400.0));
        let frame = super::super::device_link::fields("CO2: 1400ppm\nCO2_ADC: 0");
        assert_eq!(reading(&frame, &config, 1).value, None);
    }
    #[test]
    fn configuration_and_live_source_are_bound_to_the_recorded_value() {
        let root = tempfile::tempdir().unwrap();
        let mut config = infrared();
        save_settings(root.path(), &config).unwrap();
        assert_eq!(settings(root.path()), config);
        let frame =
            super::super::device_link::fields("PROTOCOL_VERSION: 2\nCO2: 1400ppm\nCO2_ADC: 160");
        publish(&root.path().join("sensors"), &frame).unwrap();
        assert_eq!(current(root.path()).unwrap().value, Some(1191.40625));
        config.reference_mv = 4900;
        save_settings(root.path(), &config).unwrap();
        assert!(current(root.path()).is_none()); // old reference must not appear current
        publish(&root.path().join("sensors"), &frame).unwrap();
        assert_eq!(current(root.path()).unwrap().value, Some(1142.578125));
        config.primary_source = "ccs811".into();
        save_settings(root.path(), &config).unwrap();
        assert!(current(root.path()).is_none()); // wait for a value from the selected input
        publish(&root.path().join("sensors"), &frame).unwrap();
        assert_eq!(current(root.path()).unwrap().value, Some(1400.0));
        config.primary_source = "analog".into();
        config.analog_model = "unidentified".into();
        assert!(save_settings(root.path(), &config).is_err());
    }
    fn kit(frame: &str) -> DeviceStatus {
        DeviceStatus {
            device_id: "SENSORKIT_MK1".into(),
            port: "/dev/ttyUSB2".into(),
            observed_at: 10000,
            error: None,
            fields: super::super::device_link::fields(frame),
        }
    }
    #[test]
    fn raw_160_counts_is_never_160_ppm_or_averaged_with_digital() {
        let d = diagnostics(&[kit("PROTOCOL_VERSION: 2\nCCS811_STATUS: ok\nCO2: 1200ppm\nCO2_ADC: 160\nDHT_STATUS: read_error\nUPTIME_S: 3600")], 10002);
        assert_eq!(d.digital_eco2_ppm, Some(1200.0));
        assert_eq!(d.reported_ppm, Some(1200.0));
        assert_eq!(d.analog_adc, Some(160));
        assert_eq!(d.analog_millivolts, Some(781.25));
        assert_eq!(d.analog_status, "unidentified");
        assert_eq!(d.compensation, "unavailable");
        assert!(!d.startup_warmup);
    }
    #[test]
    fn stale_failed_and_future_reports_do_not_present_live_readings() {
        for (time, error) in [
            (10006, None),
            (9999, None),
            (10001, Some("Live report failed: USB disconnected".into())),
        ] {
            let mut device =
                kit("PROTOCOL_VERSION: 2\nCCS811_STATUS: ok\nCO2: 1200ppm\nCO2_ADC: 160");
            device.error = error;
            let d = diagnostics(&[device], time);
            assert!(!d.fresh);
            assert_eq!(d.digital_eco2_ppm, None);
            assert_eq!(d.analog_adc, None);
        }
    }
    #[test]
    fn legacy_mixed_value_is_not_attributed_to_the_digital_sensor() {
        let d = diagnostics(&[kit("FIRMWARE_VERSION: 001\nCO2: 640ppm")], 10001);
        assert_eq!(d.source, "legacy_mixed");
        assert_eq!(d.reported_ppm, Some(640.0));
        assert_eq!(d.digital_eco2_ppm, None);
        assert_eq!(d.analog_adc, None);
    }
    #[test]
    fn malformed_unavailable_and_rail_readings_are_explicit() {
        for (raw, status) in [
            ("0", "near_ground"),
            ("1023", "near_supply"),
            ("1024", "unavailable"),
            ("NaN", "unavailable"),
        ] {
            let d = diagnostics(
                &[kit(&format!(
                    "PROTOCOL_VERSION: 2\nCO2: NaNppm\nCO2_ADC: {raw}\nCCS811_STATUS: unavailable"
                ))],
                10001,
            );
            assert_eq!(d.analog_status, status);
            assert_eq!(d.digital_eco2_ppm, None);
        }
        assert_eq!(diagnostics(&[], 10001).source, "unknown");
    }
}
