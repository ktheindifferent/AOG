// Copyright (c) 2020-2025 AOG Project Contributors
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

use serde::{Deserialize, Serialize};
use serial2::SerialPort;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const PH_OPTIMAL_MIN: f32 = 6.5;
pub const PH_OPTIMAL_MAX: f32 = 7.5;
pub const PH_CRITICAL_MIN: f32 = 5.5;
pub const PH_CRITICAL_MAX: f32 = 8.5;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhAlertLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhTrend {
    Rising,
    Stable,
    Falling,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PhCalibrationPoint {
    pub ph_value: f32,
    pub raw_value: f32,
    pub temperature: f32,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PhCalibration {
    pub point_4: Option<PhCalibrationPoint>,
    pub point_7: Option<PhCalibrationPoint>,
    pub point_10: Option<PhCalibrationPoint>,
    pub slope: f32,
    pub offset: f32,
    pub last_calibration: u64,
}

impl Default for PhCalibration {
    fn default() -> Self {
        PhCalibration {
            point_4: None,
            point_7: None,
            point_10: None,
            slope: 1.0,
            offset: 0.0,
            last_calibration: 0,
        }
    }
}

impl PhCalibration {
    pub fn point_count(&self) -> usize {
        [&self.point_4, &self.point_7, &self.point_10]
            .iter()
            .filter(|point| point.is_some())
            .count()
    }

    pub fn calculate_coefficients(&mut self) {
        self.slope = 1.0;
        self.offset = 0.0;
        if let (Some(p4), Some(p7)) = (&self.point_4, &self.point_7) {
            self.slope = (p7.ph_value - p4.ph_value) / (p7.raw_value - p4.raw_value);
            self.offset = p7.ph_value - (self.slope * p7.raw_value);
        } else if let (Some(p7), Some(p10)) = (&self.point_7, &self.point_10) {
            self.slope = (p10.ph_value - p7.ph_value) / (p10.raw_value - p7.raw_value);
            self.offset = p7.ph_value - (self.slope * p7.raw_value);
        } else if let (Some(p4), Some(p10)) = (&self.point_4, &self.point_10) {
            self.slope = (p10.ph_value - p4.ph_value) / (p10.raw_value - p4.raw_value);
            self.offset = p4.ph_value - (self.slope * p4.raw_value);
        } else if let Some(point) = self
            .point_7
            .as_ref()
            .or(self.point_4.as_ref())
            .or(self.point_10.as_ref())
        {
            // A single buffer corrects the offset immediately; a second
            // distinct buffer is needed to determine the probe's slope.
            self.offset = point.ph_value - point.raw_value;
        }
    }

    pub fn apply_calibration(&self, raw_value: f32) -> f32 {
        if let (Some(p4), Some(p7), Some(p10)) = (&self.point_4, &self.point_7, &self.point_10) {
            // Use both calibrated segments so all three saved buffers are
            // honored, including probes whose raw response runs in reverse.
            let other = if (raw_value - p7.raw_value) * (p4.raw_value - p7.raw_value) >= 0.0 {
                p4
            } else {
                p10
            };
            return p7.ph_value
                + (raw_value - p7.raw_value) * (other.ph_value - p7.ph_value)
                    / (other.raw_value - p7.raw_value);
        }
        (raw_value * self.slope) + self.offset
    }

    pub fn apply_temperature_compensation(&self, ph_value: f32, temperature: f32) -> f32 {
        let temp_coefficient = 0.003;
        let reference_temp = self
            .point_7
            .as_ref()
            .or(self.point_4.as_ref())
            .or(self.point_10.as_ref())
            .map(|point| point.temperature)
            .unwrap_or(25.0);
        ph_value + (temp_coefficient * (reference_temp - temperature))
    }

    fn value(&self, raw: f32, temperature: f32) -> f32 {
        self.apply_temperature_compensation(self.apply_calibration(raw), temperature)
    }

    pub fn warning(&self) -> Option<String> {
        let points: Vec<_> = [&self.point_4, &self.point_7, &self.point_10]
            .into_iter()
            .flatten()
            .collect();
        for (index, point) in points.iter().enumerate() {
            for other in points.iter().skip(index + 1) {
                let separation = (point.raw_value - other.raw_value).abs();
                if separation < 0.1 {
                    return Some(format!("The pH {:.0} and pH {:.0} buffers have raw readings only {:.3} apart. Small raw fluctuations will cause large pH changes. Recheck these buffers or remove the point you want to replace.", point.ph_value, other.ph_value, separation));
                }
            }
        }
        None
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PhReading {
    pub ph_value: f32,
    pub raw_value: f32,
    pub temperature: f32,
    pub timestamp: u64,
    pub alert_level: PhAlertLevel,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PhHistory {
    pub readings: VecDeque<PhReading>,
    pub max_size: usize,
}

impl Default for PhHistory {
    fn default() -> Self {
        PhHistory {
            readings: VecDeque::new(),
            max_size: 1440,
        }
    }
}

impl PhHistory {
    pub fn add_reading(&mut self, reading: PhReading) {
        if self.max_size == 0 {
            return;
        }
        while self.readings.len() >= self.max_size {
            self.readings.pop_front();
        }
        self.readings.push_back(reading);
    }

    pub fn get_trend(&self, window_minutes: usize) -> PhTrend {
        if self.readings.len() < 2 {
            return PhTrend::Stable;
        }

        let cutoff = self
            .readings
            .back()
            .unwrap()
            .timestamp
            .saturating_sub((window_minutes as u64).saturating_mul(60));
        let recent_readings: Vec<f32> = self
            .readings
            .iter()
            .filter(|reading| reading.timestamp >= cutoff)
            .map(|r| r.ph_value)
            .collect();
        let window_size = recent_readings.len();

        if recent_readings.len() < 2 {
            return PhTrend::Stable;
        }

        let first_avg =
            recent_readings.iter().take(window_size / 2).sum::<f32>() / (window_size / 2) as f32;
        let second_avg = recent_readings.iter().skip(window_size / 2).sum::<f32>()
            / (window_size - window_size / 2) as f32;

        if (second_avg - first_avg).abs() < 0.1 {
            PhTrend::Stable
        } else if second_avg > first_avg {
            PhTrend::Rising
        } else {
            PhTrend::Falling
        }
    }

    pub fn get_average(&self, window_minutes: usize) -> Option<f32> {
        if self.readings.is_empty() || window_minutes == 0 {
            return None;
        }

        let cutoff = self
            .readings
            .back()
            .unwrap()
            .timestamp
            .saturating_sub((window_minutes as u64).saturating_mul(60));
        let recent: Vec<_> = self
            .readings
            .iter()
            .filter(|reading| reading.timestamp >= cutoff)
            .map(|r| r.ph_value)
            .collect();

        Some(recent.iter().sum::<f32>() / recent.len() as f32)
    }
}

pub struct PhSensor {
    calibration: Arc<Mutex<PhCalibration>>,
    history: Arc<Mutex<PhHistory>>,
    sensor_type: PhSensorType,
    directory: PathBuf,
    config: Mutex<crate::PhConfig>,
    last_error: Mutex<Option<String>>,
}

#[derive(Debug, Clone)]
pub enum PhSensorType {
    Serial(String),
    I2C(u8),
    Arduino,
}

impl PhSensor {
    pub fn new(sensor_type: PhSensorType) -> Self {
        Self::new_in(sensor_type, Path::new("/opt/aog"))
    }

    pub(crate) fn new_in(sensor_type: PhSensorType, directory: &Path) -> Self {
        let mut calibration: PhCalibration =
            std::fs::read_to_string(directory.join("ph_calibration.json"))
                .ok()
                .and_then(|data| serde_json::from_str(&data).ok())
                .unwrap_or_default();
        // Upgrade saved single-point calibrations that previously had no effect.
        calibration.calculate_coefficients();
        let history = std::fs::read_to_string(directory.join("ph_history.json"))
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();

        PhSensor {
            calibration: Arc::new(Mutex::new(calibration)),
            history: Arc::new(Mutex::new(history)),
            sensor_type,
            directory: directory.into(),
            config: Mutex::new(crate::PhConfig::default()),
            last_error: Mutex::new(None),
        }
    }

    pub fn configure(&self, config: crate::PhConfig) {
        let mut history = self.history.lock().unwrap();
        history.max_size = config.history_size;
        while history.readings.len() > history.max_size {
            history.readings.pop_front();
        }
        drop(history);
        *self.config.lock().unwrap() = config;
    }

    pub fn history(&self) -> PhHistory {
        let calibration = self.calibration.lock().unwrap();
        let config = self.config.lock().unwrap().clone();
        self.projected_history(&calibration, &config)
    }

    fn projected_history(
        &self,
        calibration: &PhCalibration,
        config: &crate::PhConfig,
    ) -> PhHistory {
        let mut history = self.history.lock().unwrap().clone();
        for reading in &mut history.readings {
            reading.ph_value = calibration.value(reading.raw_value, reading.temperature);
            reading.alert_level = Self::alert_level_with_config(reading.ph_value, config);
        }
        history
    }
    pub fn calibration(&self) -> PhCalibration {
        self.calibration.lock().unwrap().clone()
    }
    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().clone()
    }

    fn save_calibration(&self, calibration: &PhCalibration) -> Result<(), std::io::Error> {
        let path = self.directory.join("ph_calibration.json");
        let json = serde_json::to_string_pretty(calibration)?;
        crate::atomic_write(&path, json.as_bytes())?;
        Ok(())
    }

    fn save_history(&self, history: &PhHistory) -> Result<(), std::io::Error> {
        let path = self.directory.join("ph_history.json");
        let json = serde_json::to_string_pretty(history)?;
        crate::atomic_write(&path, json.as_bytes())?;
        Ok(())
    }

    pub fn read_raw_value(&self) -> Result<f32, String> {
        match &self.sensor_type {
            PhSensorType::Arduino => {
                let path = self.directory.join("sensors/ph");
                let age = std::fs::metadata(&path)
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|modified| modified.elapsed().ok());
                if !age.is_some_and(|age| age < Duration::from_secs(10)) {
                    return Err("pH sensor reading is missing or stale".into());
                }
                std::fs::read_to_string(path)
                    .map_err(|e| e.to_string())?
                    .trim()
                    .parse::<f32>()
                    .map_err(|e| format!("Failed to parse pH value: {}", e))
            }
            PhSensorType::Serial(port) => self.read_serial_sensor(port),
            PhSensorType::I2C(address) => self.read_i2c_sensor(*address),
        }
    }

    fn read_serial_sensor(&self, port: &str) -> Result<f32, String> {
        let serial_port = SerialPort::open(port, 9600)
            .map_err(|e| format!("Failed to open serial port: {}", e))?;

        serial_port
            .write(b"R\r")
            .map_err(|e| format!("Failed to write to sensor: {}", e))?;

        std::thread::sleep(Duration::from_millis(600));

        let mut buffer = vec![0; 32];
        let bytes_read = serial_port
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read from sensor: {}", e))?;

        let response = String::from_utf8_lossy(&buffer[..bytes_read]);
        response
            .trim()
            .parse::<f32>()
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    fn read_i2c_sensor(&self, _address: u8) -> Result<f32, String> {
        Err("I2C pH sensor support not yet implemented".to_string())
    }

    pub fn get_temperature(&self) -> f32 {
        if let Ok(data) = std::fs::read_to_string(self.directory.join("sensors/temp")) {
            if let Ok(temp) = data
                .trim()
                .trim_end_matches('C')
                .trim_end_matches('°')
                .trim()
                .parse::<f32>()
            {
                if !temp.is_finite() {
                    return 25.0;
                }
                return temp;
            }
        }
        25.0
    }

    pub fn read_ph(&self) -> Result<PhReading, String> {
        let raw_value = self.read_raw_value()?;
        if !raw_value.is_finite() {
            return Err("Invalid non-finite pH reading".into());
        }
        let temperature = self.get_temperature();

        let calibration = self.calibration.lock().unwrap();
        // Keep this lock until the sample is committed: an in-flight read must
        // not publish an old calibration after a calibration change returns.
        let ph_value = calibration.value(raw_value, temperature);

        if !ph_value.is_finite() || !(0.0..=14.0).contains(&ph_value) {
            return Err("Calibrated pH is outside 0–14; check the probe and calibration".into());
        }
        let config = self.config.lock().unwrap().clone();
        let alert_level = Self::alert_level_with_config(ph_value, &config);

        let reading = PhReading {
            ph_value,
            raw_value,
            temperature,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            alert_level,
        };

        let mut history = self.history.lock().unwrap();
        history.add_reading(reading.clone());
        self.save_history(&history)
            .map_err(|e| format!("Cannot save pH history: {}", e))?;
        drop(history);

        crate::atomic_write(
            &self.directory.join("sensors/ph_calibrated"),
            format!("{:.2}", ph_value).as_bytes(),
        )
        .map_err(|error| format!("Cannot publish calibrated pH: {}", error))?;

        if config.alert_enabled && alert_level != PhAlertLevel::Normal {
            log::warn!("pH Alert: {:?} - pH value: {:.2}", alert_level, ph_value);
        }

        Ok(reading)
    }

    /// Classify a pH reading using the optimal and critical thresholds.
    pub fn calculate_alert_level(ph_value: f32) -> PhAlertLevel {
        Self::alert_level_with_config(ph_value, &crate::PhConfig::default())
    }

    fn alert_level_with_config(ph_value: f32, config: &crate::PhConfig) -> PhAlertLevel {
        if ph_value <= config.critical_min || ph_value >= config.critical_max {
            PhAlertLevel::Critical
        } else if ph_value < config.optimal_min || ph_value > config.optimal_max {
            PhAlertLevel::Warning
        } else {
            PhAlertLevel::Normal
        }
    }

    pub fn calibrate(&self, buffer_ph: f32, raw_value: f32) -> Result<(), String> {
        if !buffer_ph.is_finite() || !raw_value.is_finite() {
            return Err("Calibration values must be finite numbers".into());
        }
        let temperature = self.get_temperature();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let calibration_point = PhCalibrationPoint {
            ph_value: buffer_ph,
            raw_value,
            temperature,
            timestamp,
        };

        let mut calibration = self.calibration.lock().unwrap();
        let mut updated = calibration.clone();

        if (buffer_ph - 4.0).abs() < 0.5 {
            updated.point_4 = Some(calibration_point);
        } else if (buffer_ph - 7.0).abs() < 0.5 {
            updated.point_7 = Some(calibration_point);
        } else if (buffer_ph - 10.0).abs() < 0.5 {
            updated.point_10 = Some(calibration_point);
        } else {
            return Err(format!(
                "Invalid buffer pH value: {}. Use 4.0, 7.0, or 10.0",
                buffer_ph
            ));
        }

        let points: Vec<_> = [&updated.point_4, &updated.point_7, &updated.point_10]
            .into_iter()
            .flatten()
            .collect();
        for (index, point) in points.iter().enumerate() {
            if points
                .iter()
                .skip(index + 1)
                .any(|other| (point.raw_value - other.raw_value).abs() < 0.0001)
            {
                return Err("Calibration points need different raw readings; place the probe in the selected buffer".into());
            }
        }
        if let (Some(p4), Some(p7), Some(p10)) =
            (&updated.point_4, &updated.point_7, &updated.point_10)
        {
            if (p7.raw_value - p4.raw_value) * (p10.raw_value - p7.raw_value) <= 0.0 {
                return Err("The three buffers must produce raw readings in a consistent order. Recheck or remove the point you want to replace.".into());
            }
        }
        updated.calculate_coefficients();
        if !updated.slope.is_finite() || !updated.offset.is_finite() {
            return Err("Invalid calibration coefficients".into());
        }
        updated.last_calibration = timestamp;

        self.save_calibration(&updated)
            .map_err(|e| format!("Failed to save calibration: {}", e))?;
        *calibration = updated;
        drop(calibration);
        self.refresh_after_calibration();
        Ok(())
    }

    pub fn remove_calibration_point(&self, buffer_ph: u8) -> Result<(), String> {
        let mut calibration = self.calibration.lock().unwrap();
        let mut updated = calibration.clone();
        match buffer_ph {
            4 => updated.point_4 = None,
            7 => updated.point_7 = None,
            10 => updated.point_10 = None,
            _ => return Err("Choose the pH 4, 7, or 10 point to remove".into()),
        }
        updated.calculate_coefficients();
        updated.last_calibration = if updated.point_count() == 0 {
            0
        } else {
            chrono::Utc::now().timestamp().max(0) as u64
        };
        self.save_calibration(&updated)
            .map_err(|error| format!("Cannot save calibration: {}", error))?;
        *calibration = updated;
        drop(calibration);
        self.refresh_after_calibration();
        Ok(())
    }

    fn refresh_after_calibration(&self) {
        // Saving a calibration is still successful if the sensor is offline.
        // Report that separately and re-project saved raw samples immediately.
        let error = self.read_ph().err();
        *self.last_error.lock().unwrap() = error;
    }

    pub fn get_adjustment_suggestion(&self) -> String {
        let config = self.config.lock().unwrap().clone();
        let mut history = self.history();
        history.readings.retain(|reading| {
            reading.ph_value.is_finite() && (0.0..=14.0).contains(&reading.ph_value)
        });
        Self::adjustment_suggestion_from_history(&history, &config)
    }

    fn adjustment_suggestion_from_history(history: &PhHistory, config: &crate::PhConfig) -> String {
        if !config.auto_adjustment_enabled {
            return String::new();
        }
        if let Some(avg_ph) = history.get_average(60) {
            let trend = history.get_trend(60);

            let mut suggestion = String::new();

            if avg_ph < config.optimal_min {
                suggestion.push_str(&format!("pH too low ({:.2}). ", avg_ph));
                suggestion.push_str("Add pH UP solution (sodium bicarbonate) gradually. ");
            } else if avg_ph > config.optimal_max {
                suggestion.push_str(&format!("pH too high ({:.2}). ", avg_ph));
                suggestion.push_str("Add pH DOWN solution (phosphoric acid) gradually. ");
            } else {
                suggestion.push_str(&format!(
                    "pH is optimal ({:.2}). No adjustment needed.",
                    avg_ph
                ));
            }

            match trend {
                PhTrend::Rising => suggestion.push_str(" pH is trending upward."),
                PhTrend::Falling => suggestion.push_str(" pH is trending downward."),
                PhTrend::Stable => suggestion.push_str(" pH is stable."),
            }

            suggestion
        } else {
            "Insufficient data for pH adjustment suggestions.".to_string()
        }
    }

    pub fn get_status(&self) -> PhSensorStatus {
        let config = self.config.lock().unwrap().clone();
        let calibration = self.calibration.lock().unwrap();
        let history = self.projected_history(&calibration, &config);
        Self::status_from_history(&history, &calibration, &config)
    }

    fn status_from_history(
        history: &PhHistory,
        calibration: &PhCalibration,
        config: &crate::PhConfig,
    ) -> PhSensorStatus {
        let current = history.readings.back().filter(|reading| {
            reading.ph_value.is_finite() && (0.0..=14.0).contains(&reading.ph_value)
        });
        let mut valid = history.clone();
        valid.readings.retain(|reading| {
            reading.ph_value.is_finite() && (0.0..=14.0).contains(&reading.ph_value)
        });
        PhSensorStatus {
            current_ph: current.map(|r| r.ph_value),
            average_ph: valid.get_average(60),
            trend: valid.get_trend(60),
            alert_level: current
                .map(|r| Self::alert_level_with_config(r.ph_value, &config))
                .unwrap_or(PhAlertLevel::Normal),
            last_calibration: calibration.last_calibration,
            calibration_valid: calibration.point_count() >= 2,
            calibration_points: calibration.point_count(),
            adjustment_suggestion: Self::adjustment_suggestion_from_history(&valid, &config),
            last_reading: history.readings.back().map(|reading| reading.timestamp),
            reading_count: history.readings.len(),
            enabled: config.enabled,
        }
    }

    pub fn snapshot(&self) -> serde_json::Value {
        let config = self.config.lock().unwrap().clone();
        let raw = self.read_raw_value();
        let temperature = self.get_temperature();
        let calibration = self.calibration.lock().unwrap();
        let history = self.projected_history(&calibration, &config);
        let mut status = Self::status_from_history(&history, &calibration, &config);
        let mut error = self.last_error();
        let raw_value = raw.as_ref().ok().copied().filter(|raw| raw.is_finite());
        if let Some(raw) = raw_value {
            let value = calibration.value(raw, temperature);
            status.current_ph =
                (value.is_finite() && (0.0..=14.0).contains(&value)).then_some(value);
            status.last_reading = Some(chrono::Utc::now().timestamp().max(0) as u64);
            status.alert_level = Self::alert_level_with_config(value, &config);
            if status.current_ph.is_none() {
                error = Some(
                    "Calibrated pH is outside 0–14; check the probe and saved calibration points"
                        .into(),
                );
            } else if error.as_ref().is_some_and(|message| {
                !message.starts_with("Cannot save") && !message.starts_with("Cannot publish")
            }) {
                // The live read has recovered; keep storage errors visible,
                // but do not keep showing an old sensor failure until the next sample.
                error = None;
            }
        } else if let Err(failure) = raw {
            error = Some(failure);
        }
        let readings: Vec<_> = history.readings.iter().map(|reading| serde_json::json!({
            "timestamp": reading.timestamp,
            "ph_value": (reading.ph_value.is_finite() && (0.0..=14.0).contains(&reading.ph_value)).then_some(reading.ph_value),
            "raw_value": reading.raw_value,
            "temperature": reading.temperature,
            "alert_level": reading.alert_level,
        })).collect();
        serde_json::json!({
            "status": status, "history": readings, "calibration": *calibration,
            "calibration_warning": calibration.warning(), "error": error, "config": config,
            "raw_value": raw_value, "server_time": chrono::Utc::now().timestamp(),
            "history_uses_current_calibration": true,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PhSensorStatus {
    pub current_ph: Option<f32>,
    pub average_ph: Option<f32>,
    pub trend: PhTrend,
    pub alert_level: PhAlertLevel,
    pub last_calibration: u64,
    pub calibration_valid: bool,
    #[serde(default)]
    pub calibration_points: usize,
    pub adjustment_suggestion: String,
    pub last_reading: Option<u64>,
    pub reading_count: usize,
    pub enabled: bool,
}

pub fn active_sensor() -> Arc<PhSensor> {
    static SENSOR: OnceLock<Arc<PhSensor>> = OnceLock::new();
    Arc::clone(SENSOR.get_or_init(|| Arc::new(PhSensor::new(PhSensorType::Arduino))))
}

pub fn init_ph_monitoring() {
    use std::thread;

    let _ = thread::Builder::new()
        .name("ph_monitoring_thread".to_string())
        .spawn(move || {
            let sensor = active_sensor();
            if let Ok(config) = crate::Config::load(0) {
                sensor.configure(config.ph_config.unwrap_or_default());
            }
            let mut last_reading: Option<Instant> = None;

            loop {
                let config = sensor.config.lock().unwrap().clone();
                let interval =
                    Duration::from_secs(config.monitoring_interval_seconds.clamp(5, 3600));
                if !config.enabled || last_reading.is_some_and(|last| last.elapsed() < interval) {
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }
                match sensor.read_ph() {
                    Ok(reading) => {
                        *sensor.last_error.lock().unwrap() = None;
                        log::info!(
                            "pH Reading: {:.2} (raw: {:.2}, temp: {:.1}°C)",
                            reading.ph_value,
                            reading.raw_value,
                            reading.temperature
                        );

                        if config.alert_enabled && reading.alert_level == PhAlertLevel::Critical {
                            log::error!(
                                "CRITICAL pH ALERT: pH value {:.2} is outside safe range!",
                                reading.ph_value
                            );
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to read pH sensor: {}", e);
                        *sensor.last_error.lock().unwrap() = Some(e);
                    }
                }

                last_reading = Some(Instant::now());
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, PhSensor) {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("sensors")).unwrap();
        let sensor = PhSensor::new_in(PhSensorType::Arduino, directory.path());
        (directory, sensor)
    }

    #[test]
    fn history_windows_use_elapsed_minutes_and_resizing_keeps_newest() {
        let (_directory, sensor) = fixture();
        for (timestamp, ph_value) in [(0, 4.0), (600, 7.0), (630, 7.0), (660, 7.0)] {
            sensor.history.lock().unwrap().add_reading(PhReading {
                timestamp,
                ph_value,
                raw_value: ph_value,
                temperature: 25.0,
                alert_level: PhAlertLevel::Normal,
            });
        }
        assert_eq!(sensor.history().get_average(1), Some(7.0));
        assert_eq!(sensor.history().get_trend(1), PhTrend::Stable);
        assert_eq!(sensor.history().get_average(0), None);
        sensor.configure(crate::PhConfig {
            history_size: 2,
            ..Default::default()
        });
        assert_eq!(sensor.history().readings.front().unwrap().timestamp, 630);
        sensor.configure(crate::PhConfig {
            history_size: 0,
            ..Default::default()
        });
        assert!(sensor.history().readings.is_empty());
    }

    #[test]
    fn reads_temperature_units_and_applies_live_alert_thresholds() {
        let (directory, sensor) = fixture();
        std::fs::write(directory.path().join("sensors/ph"), "6.0").unwrap();
        for units in ["20.0C", "20.0°C"] {
            std::fs::write(directory.path().join("sensors/temp"), units).unwrap();
            assert_eq!(sensor.get_temperature(), 20.0);
        }
        sensor.configure(crate::PhConfig {
            critical_min: 4.0,
            optimal_min: 5.0,
            optimal_max: 6.5,
            critical_max: 8.0,
            ..Default::default()
        });
        let reading = sensor.read_ph().unwrap();
        assert!((reading.ph_value - 6.015).abs() < 0.001);
        assert_eq!(reading.alert_level, PhAlertLevel::Normal);
        assert_eq!(sensor.get_status().alert_level, PhAlertLevel::Normal);
        let reloaded = PhSensor::new_in(PhSensorType::Arduino, directory.path());
        assert_eq!(reloaded.history().readings.len(), 1);
    }

    #[test]
    fn stale_missing_and_nonfinite_readings_do_not_enter_history() {
        let (directory, sensor) = fixture();
        assert!(sensor.read_ph().is_err());
        let path = directory.path().join("sensors/ph");
        std::fs::write(&path, "NaN").unwrap();
        assert!(sensor.read_ph().is_err());
        std::fs::write(&path, "7.0").unwrap();
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(11))
            .unwrap();
        assert!(sensor.read_ph().unwrap_err().contains("stale"));
        assert!(sensor.history().readings.is_empty());
    }

    #[test]
    fn invalid_calibration_preserves_saved_points_and_valid_pair_survives_reload() {
        let (directory, sensor) = fixture();
        sensor.calibrate(7.0, 200.0).unwrap();
        assert!(!sensor.get_status().calibration_valid);
        let path = directory.path().join("ph_calibration.json");
        let before = std::fs::read(&path).unwrap();
        assert!(sensor.calibrate(4.0, 200.0).is_err());
        assert!(sensor.calibrate(4.0, f32::NAN).is_err());
        assert!(sensor.calibrate(6.0, 100.0).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(sensor.calibration().point_4.is_none());
        sensor.calibrate(4.0, 100.0).unwrap();
        let reloaded = PhSensor::new_in(PhSensorType::Arduino, directory.path());
        assert!(reloaded.get_status().calibration_valid);
        assert!((reloaded.calibration().apply_calibration(150.0) - 5.5).abs() < 0.001);
    }

    #[test]
    fn test_ph_alert_levels() {
        assert_eq!(PhSensor::calculate_alert_level(7.0), PhAlertLevel::Normal);
        assert_eq!(PhSensor::calculate_alert_level(6.5), PhAlertLevel::Normal);
        assert_eq!(PhSensor::calculate_alert_level(7.5), PhAlertLevel::Normal);

        assert_eq!(PhSensor::calculate_alert_level(6.0), PhAlertLevel::Warning);
        assert_eq!(PhSensor::calculate_alert_level(8.0), PhAlertLevel::Warning);

        assert_eq!(PhSensor::calculate_alert_level(5.0), PhAlertLevel::Critical);
        assert_eq!(PhSensor::calculate_alert_level(9.0), PhAlertLevel::Critical);
    }

    #[test]
    fn test_calibration_calculation() {
        let mut cal = PhCalibration::default();

        cal.point_4 = Some(PhCalibrationPoint {
            ph_value: 4.0,
            raw_value: 100.0,
            temperature: 25.0,
            timestamp: 0,
        });

        cal.point_7 = Some(PhCalibrationPoint {
            ph_value: 7.0,
            raw_value: 200.0,
            temperature: 25.0,
            timestamp: 0,
        });

        cal.calculate_coefficients();

        assert!((cal.slope - 0.03).abs() < 0.001);
        assert!((cal.offset - 1.0).abs() < 0.001);

        let calibrated = cal.apply_calibration(150.0);
        assert!((calibrated - 5.5).abs() < 0.01);
    }

    #[test]
    fn test_temperature_compensation() {
        let cal = PhCalibration::default();

        let ph_at_25c = 7.0;
        let ph_at_20c = cal.apply_temperature_compensation(ph_at_25c, 20.0);
        assert!((ph_at_20c - 7.015).abs() < 0.001);

        let ph_at_30c = cal.apply_temperature_compensation(ph_at_25c, 30.0);
        assert!((ph_at_30c - 6.985).abs() < 0.001);
    }

    #[test]
    fn test_ph_history_trend() {
        let mut history = PhHistory::default();

        for i in 0..10 {
            history.add_reading(PhReading {
                ph_value: 7.0 + (i as f32 * 0.1),
                raw_value: 200.0,
                temperature: 25.0,
                timestamp: i,
                alert_level: PhAlertLevel::Normal,
            });
        }

        assert_eq!(history.get_trend(5), PhTrend::Rising);

        history.readings.clear();

        for i in 0..10 {
            history.add_reading(PhReading {
                ph_value: 7.5 - (i as f32 * 0.1),
                raw_value: 200.0,
                temperature: 25.0,
                timestamp: i,
                alert_level: PhAlertLevel::Normal,
            });
        }

        assert_eq!(history.get_trend(5), PhTrend::Falling);
    }
}
