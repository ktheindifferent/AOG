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

use serde::{Serialize, Deserialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::collections::VecDeque;
use serial2::SerialPort;

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
    pub fn calculate_coefficients(&mut self) {
        if let (Some(p4), Some(p7)) = (&self.point_4, &self.point_7) {
            self.slope = (p7.ph_value - p4.ph_value) / (p7.raw_value - p4.raw_value);
            self.offset = p7.ph_value - (self.slope * p7.raw_value);
        } else if let (Some(p7), Some(p10)) = (&self.point_7, &self.point_10) {
            self.slope = (p10.ph_value - p7.ph_value) / (p10.raw_value - p7.raw_value);
            self.offset = p7.ph_value - (self.slope * p7.raw_value);
        }
    }
    
    pub fn apply_calibration(&self, raw_value: f32) -> f32 {
        (raw_value * self.slope) + self.offset
    }
    
    pub fn apply_temperature_compensation(&self, ph_value: f32, temperature: f32) -> f32 {
        let temp_coefficient = 0.003;
        let reference_temp = 25.0;
        ph_value + (temp_coefficient * (reference_temp - temperature))
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
        if self.readings.len() >= self.max_size {
            self.readings.pop_front();
        }
        self.readings.push_back(reading);
    }
    
    pub fn get_trend(&self, window_minutes: usize) -> PhTrend {
        if self.readings.len() < 2 {
            return PhTrend::Stable;
        }
        
        let window_size = window_minutes.min(self.readings.len());
        let recent_readings: Vec<f32> = self.readings
            .iter()
            .rev()
            .take(window_size)
            .map(|r| r.ph_value)
            .collect();
        
        if recent_readings.len() < 2 {
            return PhTrend::Stable;
        }
        
        let first_avg = recent_readings.iter().take(window_size / 2).sum::<f32>() / (window_size / 2) as f32;
        let second_avg = recent_readings.iter().skip(window_size / 2).sum::<f32>() / (window_size - window_size / 2) as f32;
        
        if (second_avg - first_avg).abs() < 0.1 {
            PhTrend::Stable
        } else if second_avg > first_avg {
            PhTrend::Rising
        } else {
            PhTrend::Falling
        }
    }
    
    pub fn get_average(&self, window_minutes: usize) -> Option<f32> {
        if self.readings.is_empty() {
            return None;
        }
        
        let window_size = window_minutes.min(self.readings.len());
        let sum: f32 = self.readings
            .iter()
            .rev()
            .take(window_size)
            .map(|r| r.ph_value)
            .sum();
        
        Some(sum / window_size as f32)
    }
}

pub struct PhSensor {
    calibration: Arc<Mutex<PhCalibration>>,
    history: Arc<Mutex<PhHistory>>,
    sensor_type: PhSensorType,
}

#[derive(Debug, Clone)]
pub enum PhSensorType {
    Serial(String),
    I2C(u8),
    Arduino,
}

impl PhSensor {
    pub fn new(sensor_type: PhSensorType) -> Self {
        let calibration = Self::load_calibration();
        let history = Self::load_history();
        
        PhSensor {
            calibration: Arc::new(Mutex::new(calibration)),
            history: Arc::new(Mutex::new(history)),
            sensor_type,
        }
    }
    
    fn load_calibration() -> PhCalibration {
        let path = "/opt/aog/ph_calibration.json";
        if Path::new(path).exists() {
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(cal) = serde_json::from_str(&data) {
                    return cal;
                }
            }
        }
        PhCalibration::default()
    }
    
    fn save_calibration(calibration: &PhCalibration) -> Result<(), std::io::Error> {
        let path = "/opt/aog/ph_calibration.json";
        let json = serde_json::to_string_pretty(calibration)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    
    fn load_history() -> PhHistory {
        let path = "/opt/aog/ph_history.json";
        if Path::new(path).exists() {
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(history) = serde_json::from_str(&data) {
                    return history;
                }
            }
        }
        PhHistory::default()
    }
    
    fn save_history(history: &PhHistory) -> Result<(), std::io::Error> {
        let path = "/opt/aog/ph_history.json";
        let json = serde_json::to_string_pretty(history)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    
    pub fn read_raw_value(&self) -> Result<f32, String> {
        match &self.sensor_type {
            PhSensorType::Arduino => {
                if let Ok(data) = std::fs::read_to_string("/opt/aog/sensors/ph") {
                    data.trim().parse::<f32>()
                        .map_err(|e| format!("Failed to parse pH value: {}", e))
                } else {
                    Err("Failed to read pH sensor file".to_string())
                }
            },
            PhSensorType::Serial(port) => {
                self.read_serial_sensor(port)
            },
            PhSensorType::I2C(address) => {
                self.read_i2c_sensor(*address)
            }
        }
    }
    
    fn read_serial_sensor(&self, port: &str) -> Result<f32, String> {
        let mut serial_port = SerialPort::open(port, 9600)
            .map_err(|e| format!("Failed to open serial port: {}", e))?;
        
        serial_port.write(b"R\r")
            .map_err(|e| format!("Failed to write to sensor: {}", e))?;
        
        std::thread::sleep(Duration::from_millis(600));
        
        let mut buffer = vec![0; 32];
        let bytes_read = serial_port.read(&mut buffer)
            .map_err(|e| format!("Failed to read from sensor: {}", e))?;
        
        let response = String::from_utf8_lossy(&buffer[..bytes_read]);
        response.trim()
            .parse::<f32>()
            .map_err(|e| format!("Failed to parse response: {}", e))
    }
    
    fn read_i2c_sensor(&self, _address: u8) -> Result<f32, String> {
        Err("I2C pH sensor support not yet implemented".to_string())
    }
    
    pub fn get_temperature(&self) -> f32 {
        if let Ok(data) = std::fs::read_to_string("/opt/aog/sensors/temp") {
            if let Ok(temp) = data.trim().parse::<f32>() {
                return temp;
            }
        }
        25.0
    }
    
    pub fn read_ph(&self) -> Result<PhReading, String> {
        let raw_value = self.read_raw_value()?;
        let temperature = self.get_temperature();
        
        let calibration = self.calibration.lock().unwrap();
        let mut ph_value = calibration.apply_calibration(raw_value);
        ph_value = calibration.apply_temperature_compensation(ph_value, temperature);
        drop(calibration);
        
        let alert_level = Self::calculate_alert_level(ph_value);
        
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
        let _ = Self::save_history(&history);
        drop(history);
        
        if let Ok(mut f) = File::create("/opt/aog/sensors/ph_calibrated") {
            let _ = f.write_all(format!("{:.2}", ph_value).as_bytes());
        }
        
        if alert_level != PhAlertLevel::Normal {
            log::warn!("pH Alert: {:?} - pH value: {:.2}", alert_level, ph_value);
        }
        
        Ok(reading)
    }
    
    fn calculate_alert_level(ph_value: f32) -> PhAlertLevel {
        if ph_value < PH_CRITICAL_MIN || ph_value > PH_CRITICAL_MAX {
            PhAlertLevel::Critical
        } else if ph_value < PH_OPTIMAL_MIN || ph_value > PH_OPTIMAL_MAX {
            PhAlertLevel::Warning
        } else {
            PhAlertLevel::Normal
        }
    }
    
    pub fn calibrate(&self, buffer_ph: f32, raw_value: f32) -> Result<(), String> {
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
        
        if (buffer_ph - 4.0).abs() < 0.5 {
            calibration.point_4 = Some(calibration_point);
        } else if (buffer_ph - 7.0).abs() < 0.5 {
            calibration.point_7 = Some(calibration_point);
        } else if (buffer_ph - 10.0).abs() < 0.5 {
            calibration.point_10 = Some(calibration_point);
        } else {
            return Err(format!("Invalid buffer pH value: {}. Use 4.0, 7.0, or 10.0", buffer_ph));
        }
        
        calibration.calculate_coefficients();
        calibration.last_calibration = timestamp;
        
        Self::save_calibration(&calibration)
            .map_err(|e| format!("Failed to save calibration: {}", e))?;
        
        Ok(())
    }
    
    pub fn get_adjustment_suggestion(&self) -> String {
        let history = self.history.lock().unwrap();
        
        if let Some(avg_ph) = history.get_average(60) {
            let trend = history.get_trend(60);
            
            let mut suggestion = String::new();
            
            if avg_ph < PH_OPTIMAL_MIN {
                suggestion.push_str(&format!("pH too low ({:.2}). ", avg_ph));
                suggestion.push_str("Add pH UP solution (sodium bicarbonate) gradually. ");
            } else if avg_ph > PH_OPTIMAL_MAX {
                suggestion.push_str(&format!("pH too high ({:.2}). ", avg_ph));
                suggestion.push_str("Add pH DOWN solution (phosphoric acid) gradually. ");
            } else {
                suggestion.push_str(&format!("pH is optimal ({:.2}). No adjustment needed.", avg_ph));
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
        let calibration = self.calibration.lock().unwrap();
        let history = self.history.lock().unwrap();
        
        PhSensorStatus {
            current_ph: history.readings.back().map(|r| r.ph_value),
            average_ph: history.get_average(60),
            trend: history.get_trend(60),
            alert_level: history.readings.back()
                .map(|r| r.alert_level)
                .unwrap_or(PhAlertLevel::Normal),
            last_calibration: calibration.last_calibration,
            calibration_valid: calibration.point_7.is_some(),
            adjustment_suggestion: self.get_adjustment_suggestion(),
        }
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
    pub adjustment_suggestion: String,
}

pub fn init_ph_monitoring() {
    use std::thread;
    
    let _ = thread::Builder::new()
        .name("ph_monitoring_thread".to_string())
        .spawn(move || {
            let sensor = PhSensor::new(PhSensorType::Arduino);
            
            loop {
                match sensor.read_ph() {
                    Ok(reading) => {
                        log::info!("pH Reading: {:.2} (raw: {:.2}, temp: {:.1}°C)", 
                            reading.ph_value, reading.raw_value, reading.temperature);
                        
                        if reading.alert_level == PhAlertLevel::Critical {
                            log::error!("CRITICAL pH ALERT: pH value {:.2} is outside safe range!", 
                                reading.ph_value);
                        }
                    },
                    Err(e) => {
                        log::error!("Failed to read pH sensor: {}", e);
                    }
                }
                
                thread::sleep(Duration::from_secs(60));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    
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