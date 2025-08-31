// Copyright (c) 2024 Terragon Labs
//
// MIT License
//
// Water Level Sensor Module - Provides real-time water level monitoring
// for tanks with support for multiple sensor types and safety features

use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use std::thread;
use std::fs;
use std::io::Write;
use chrono::Local;
use serde::{Deserialize, Serialize};
use rppal::gpio::{Gpio, InputPin, OutputPin, Level};
use crate::{WaterLevelConfig, WaterLevelSensorType};

/// Water level reading with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterLevelReading {
    pub tank_id: String,
    pub level_cm: f32,
    pub level_percent: f32,
    pub timestamp: String,
    pub sensor_type: WaterLevelSensorType,
    pub is_valid: bool,
    pub error_message: Option<String>,
}

/// Water level sensor trait for different sensor implementations
pub trait WaterLevelSensor: Send + Sync {
    fn read(&mut self) -> Result<f32, String>;
    fn calibrate(&mut self, actual_level_cm: f32) -> Result<(), String>;
    fn get_sensor_type(&self) -> WaterLevelSensorType;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Ultrasonic sensor implementation (HC-SR04)
pub struct UltrasonicSensor {
    trigger_pin: OutputPin,
    echo_pin: InputPin,
    calibration_offset: f32,
    calibration_factor: f32,
    timeout_ms: u64,
}

impl UltrasonicSensor {
    pub fn new(trigger_gpio: u8, echo_gpio: u8, config: &WaterLevelConfig) -> Result<Self, String> {
        let gpio = Gpio::new().map_err(|e| format!("Failed to initialize GPIO: {}", e))?;
        
        let trigger_pin = gpio.get(trigger_gpio)
            .map_err(|e| format!("Failed to get trigger pin {}: {}", trigger_gpio, e))?
            .into_output();
            
        let echo_pin = gpio.get(echo_gpio)
            .map_err(|e| format!("Failed to get echo pin {}: {}", echo_gpio, e))?
            .into_input();
            
        Ok(UltrasonicSensor {
            trigger_pin,
            echo_pin,
            calibration_offset: config.calibration_offset,
            calibration_factor: config.calibration_factor,
            timeout_ms: config.sensor_timeout_ms,
        })
    }
    
    fn measure_distance(&mut self) -> Result<f32, String> {
        // Send trigger pulse
        self.trigger_pin.set_low();
        thread::sleep(Duration::from_micros(2));
        self.trigger_pin.set_high();
        thread::sleep(Duration::from_micros(10));
        self.trigger_pin.set_low();
        
        // Wait for echo to go high
        let start_wait = Instant::now();
        while self.echo_pin.is_low() {
            if start_wait.elapsed() > Duration::from_millis(self.timeout_ms) {
                return Err("Timeout waiting for echo pulse".to_string());
            }
        }
        
        // Measure echo pulse duration
        let pulse_start = Instant::now();
        while self.echo_pin.is_high() {
            if pulse_start.elapsed() > Duration::from_millis(self.timeout_ms) {
                return Err("Timeout measuring echo pulse".to_string());
            }
        }
        let pulse_duration = pulse_start.elapsed();
        
        // Calculate distance (speed of sound = 343 m/s at room temperature)
        // Distance = (time * speed) / 2 (divide by 2 for round trip)
        let distance_cm = (pulse_duration.as_micros() as f32 * 0.0343) / 2.0;
        
        Ok((distance_cm + self.calibration_offset) * self.calibration_factor)
    }
}

impl WaterLevelSensor for UltrasonicSensor {
    fn read(&mut self) -> Result<f32, String> {
        // Take multiple readings and average them
        let mut readings = Vec::new();
        for _ in 0..3 {
            match self.measure_distance() {
                Ok(distance) => readings.push(distance),
                Err(e) => log::warn!("Ultrasonic reading failed: {}", e),
            }
            thread::sleep(Duration::from_millis(50));
        }
        
        if readings.is_empty() {
            return Err("All ultrasonic readings failed".to_string());
        }
        
        let avg = readings.iter().sum::<f32>() / readings.len() as f32;
        Ok(avg)
    }
    
    fn calibrate(&mut self, actual_level_cm: f32) -> Result<(), String> {
        let measured = self.read()?;
        self.calibration_offset = actual_level_cm - measured;
        Ok(())
    }
    
    fn get_sensor_type(&self) -> WaterLevelSensorType {
        WaterLevelSensorType::Ultrasonic
    }
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Mock sensor for testing
pub struct MockSensor {
    level: f32,
    sensor_type: WaterLevelSensorType,
}

impl MockSensor {
    pub fn new(initial_level: f32) -> Self {
        MockSensor {
            level: initial_level,
            sensor_type: WaterLevelSensorType::Mock,
        }
    }
    
    pub fn set_level(&mut self, level: f32) {
        self.level = level;
    }
}

impl WaterLevelSensor for MockSensor {
    fn read(&mut self) -> Result<f32, String> {
        Ok(self.level)
    }
    
    fn calibrate(&mut self, _actual_level_cm: f32) -> Result<(), String> {
        Ok(())
    }
    
    fn get_sensor_type(&self) -> WaterLevelSensorType {
        self.sensor_type.clone()
    }
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Water level monitor with moving average and failure detection
pub struct WaterLevelMonitor {
    tank_id: String,
    sensor: Arc<Mutex<Box<dyn WaterLevelSensor>>>,
    config: WaterLevelConfig,
    reading_history: Arc<Mutex<VecDeque<f32>>>,
    last_valid_reading: Arc<Mutex<Option<WaterLevelReading>>>,
    consecutive_failures: Arc<Mutex<u32>>,
    max_consecutive_failures: u32,
}

impl WaterLevelMonitor {
    pub fn new(
        tank_id: String,
        sensor: Box<dyn WaterLevelSensor>,
        config: WaterLevelConfig,
    ) -> Self {
        WaterLevelMonitor {
            tank_id,
            sensor: Arc::new(Mutex::new(sensor)),
            config,
            reading_history: Arc::new(Mutex::new(VecDeque::new())),
            last_valid_reading: Arc::new(Mutex::new(None)),
            consecutive_failures: Arc::new(Mutex::new(0)),
            max_consecutive_failures: 3,
        }
    }
    
    /// Get current water level with moving average
    pub fn get_level(&self) -> WaterLevelReading {
        let mut sensor = self.sensor.lock().unwrap();
        
        match sensor.read() {
            Ok(raw_level) => {
                // Reset failure counter
                *self.consecutive_failures.lock().unwrap() = 0;
                
                // Add to history for moving average
                let mut history = self.reading_history.lock().unwrap();
                history.push_back(raw_level);
                
                // Keep only the configured number of samples
                while history.len() > self.config.moving_average_samples {
                    history.pop_front();
                }
                
                // Calculate moving average
                let avg_level = if !history.is_empty() {
                    history.iter().sum::<f32>() / history.len() as f32
                } else {
                    raw_level
                };
                
                // Convert to percentage
                let level_percent = ((self.config.tank_height_cm - avg_level) / self.config.tank_height_cm * 100.0)
                    .max(0.0)
                    .min(100.0);
                
                let reading = WaterLevelReading {
                    tank_id: self.tank_id.clone(),
                    level_cm: self.config.tank_height_cm - avg_level,
                    level_percent,
                    timestamp: Local::now().to_rfc3339(),
                    sensor_type: sensor.get_sensor_type(),
                    is_valid: true,
                    error_message: None,
                };
                
                // Save as last valid reading
                *self.last_valid_reading.lock().unwrap() = Some(reading.clone());
                
                // Write to sensor file
                self.write_sensor_file(level_percent);
                
                reading
            }
            Err(e) => {
                // Increment failure counter
                let mut failures = self.consecutive_failures.lock().unwrap();
                *failures += 1;
                
                log::error!("Water level sensor {} failed: {} (failure #{}/{})", 
                    self.tank_id, e, *failures, self.max_consecutive_failures);
                
                // Check if we should use fallback
                if *failures >= self.max_consecutive_failures && self.config.enable_fallback_mode {
                    self.use_fallback_reading()
                } else if let Some(last) = self.last_valid_reading.lock().unwrap().clone() {
                    // Use last valid reading
                    WaterLevelReading {
                        is_valid: false,
                        error_message: Some(format!("Using last valid reading due to: {}", e)),
                        ..last
                    }
                } else {
                    // No valid reading available
                    WaterLevelReading {
                        tank_id: self.tank_id.clone(),
                        level_cm: 0.0,
                        level_percent: 0.0,
                        timestamp: Local::now().to_rfc3339(),
                        sensor_type: sensor.get_sensor_type(),
                        is_valid: false,
                        error_message: Some(e),
                    }
                }
            }
        }
    }
    
    /// Use overflow sensor as fallback
    fn use_fallback_reading(&self) -> WaterLevelReading {
        let ovf_value = match self.tank_id.as_str() {
            "tank1" => crate::aog::sensors::get_value("t1_ovf"),
            "tank2" => crate::aog::sensors::get_value("t2_ovf"),
            _ => "NONE".to_string(),
        };
        
        let (level_percent, level_cm) = if ovf_value.contains("OVERFLOW") {
            (95.0, self.config.max_fill_level_cm)
        } else {
            // Assume moderate level if not overflowing
            (50.0, self.config.tank_height_cm / 2.0)
        };
        
        log::warn!("Using overflow sensor fallback for {}: {}%", self.tank_id, level_percent);
        
        WaterLevelReading {
            tank_id: self.tank_id.clone(),
            level_cm,
            level_percent,
            timestamp: Local::now().to_rfc3339(),
            sensor_type: WaterLevelSensorType::Float,
            is_valid: false,
            error_message: Some("Fallback to overflow sensor".to_string()),
        }
    }
    
    /// Write level to sensor file for compatibility
    fn write_sensor_file(&self, level_percent: f32) {
        let filename = format!("/opt/aog/sensors/{}_level", self.tank_id);
        if let Ok(mut file) = fs::File::create(&filename) {
            let _ = write!(file, "{:.1}", level_percent);
        }
    }
    
    /// Calibrate the sensor
    pub fn calibrate(&self, actual_level_cm: f32) -> Result<(), String> {
        let mut sensor = self.sensor.lock().unwrap();
        sensor.calibrate(actual_level_cm)?;
        
        // Clear history after calibration
        self.reading_history.lock().unwrap().clear();
        
        log::info!("Water level sensor {} calibrated to {}cm", self.tank_id, actual_level_cm);
        Ok(())
    }
    
    /// Get sensor statistics
    pub fn get_stats(&self) -> serde_json::Value {
        let history = self.reading_history.lock().unwrap();
        let failures = self.consecutive_failures.lock().unwrap();
        
        serde_json::json!({
            "tank_id": self.tank_id,
            "sensor_type": format!("{:?}", self.sensor.lock().unwrap().get_sensor_type()),
            "samples_in_average": history.len(),
            "consecutive_failures": *failures,
            "max_failures_before_fallback": self.max_consecutive_failures,
            "fallback_enabled": self.config.enable_fallback_mode,
        })
    }
}

/// Global water level monitoring system
pub struct WaterLevelSystem {
    monitors: Arc<Mutex<Vec<WaterLevelMonitor>>>,
    config: WaterLevelConfig,
}

impl WaterLevelSystem {
    pub fn new(config: WaterLevelConfig) -> Self {
        WaterLevelSystem {
            monitors: Arc::new(Mutex::new(Vec::new())),
            config,
        }
    }
    
    /// Initialize water level monitoring for all tanks
    pub fn init(&mut self) -> Result<(), String> {
        let mut monitors = Vec::new();
        
        // Initialize tank 1 sensor
        if let Some(pin) = self.config.tank1_sensor_pin {
            let sensor: Box<dyn WaterLevelSensor> = match self.config.sensor_type {
                WaterLevelSensorType::Ultrasonic => {
                    // Assuming trigger and echo pins are consecutive
                    Box::new(UltrasonicSensor::new(pin, pin + 1, &self.config)?)
                }
                WaterLevelSensorType::Mock => {
                    Box::new(MockSensor::new(50.0))
                }
                _ => {
                    log::warn!("Sensor type {:?} not yet implemented, using mock", self.config.sensor_type);
                    Box::new(MockSensor::new(50.0))
                }
            };
            
            monitors.push(WaterLevelMonitor::new(
                "tank1".to_string(),
                sensor,
                self.config.clone(),
            ));
        }
        
        // Initialize tank 2 sensor
        if let Some(pin) = self.config.tank2_sensor_pin {
            let sensor: Box<dyn WaterLevelSensor> = match self.config.sensor_type {
                WaterLevelSensorType::Ultrasonic => {
                    Box::new(UltrasonicSensor::new(pin, pin + 1, &self.config)?)
                }
                WaterLevelSensorType::Mock => {
                    Box::new(MockSensor::new(50.0))
                }
                _ => {
                    log::warn!("Sensor type {:?} not yet implemented, using mock", self.config.sensor_type);
                    Box::new(MockSensor::new(50.0))
                }
            };
            
            monitors.push(WaterLevelMonitor::new(
                "tank2".to_string(),
                sensor,
                self.config.clone(),
            ));
        }
        
        *self.monitors.lock().unwrap() = monitors;
        
        log::info!("Water level monitoring system initialized with {} monitors", 
            self.monitors.lock().unwrap().len());
        
        Ok(())
    }
    
    /// Get water level for specific tank
    pub fn get_tank_level(&self, tank_id: &str) -> Option<WaterLevelReading> {
        let monitors = self.monitors.lock().unwrap();
        monitors.iter()
            .find(|m| m.tank_id == tank_id)
            .map(|m| m.get_level())
    }
    
    /// Get all tank levels
    pub fn get_all_levels(&self) -> Vec<WaterLevelReading> {
        let monitors = self.monitors.lock().unwrap();
        monitors.iter().map(|m| m.get_level()).collect()
    }
    
    /// Calibrate specific tank sensor
    pub fn calibrate_tank(&self, tank_id: &str, actual_level_cm: f32) -> Result<(), String> {
        let monitors = self.monitors.lock().unwrap();
        monitors.iter()
            .find(|m| m.tank_id == tank_id)
            .ok_or_else(|| format!("Tank {} not found", tank_id))?
            .calibrate(actual_level_cm)
    }
    
    /// Get system statistics
    pub fn get_stats(&self) -> serde_json::Value {
        let monitors = self.monitors.lock().unwrap();
        let stats: Vec<_> = monitors.iter().map(|m| m.get_stats()).collect();
        
        serde_json::json!({
            "monitors": stats,
            "config": {
                "sensor_type": format!("{:?}", self.config.sensor_type),
                "tank_height_cm": self.config.tank_height_cm,
                "max_fill_level_cm": self.config.max_fill_level_cm,
                "min_level_cm": self.config.min_level_cm,
                "moving_average_samples": self.config.moving_average_samples,
            }
        })
    }
}

// Global water level system instance
lazy_static::lazy_static! {
    pub static ref WATER_LEVEL_SYSTEM: Mutex<Option<WaterLevelSystem>> = Mutex::new(None);
}

/// Initialize the global water level system
pub fn init_water_level_system(config: WaterLevelConfig) -> Result<(), String> {
    let mut system = WaterLevelSystem::new(config);
    system.init()?;
    *WATER_LEVEL_SYSTEM.lock().unwrap() = Some(system);
    Ok(())
}

/// Get water level for a specific tank (percentage)
pub fn get_water_level_percent(tank_id: &str) -> f32 {
    if let Some(system) = WATER_LEVEL_SYSTEM.lock().unwrap().as_ref() {
        if let Some(reading) = system.get_tank_level(tank_id) {
            return reading.level_percent;
        }
    }
    
    // Fallback to overflow sensor check
    let ovf_value = match tank_id {
        "tank1" => crate::aog::sensors::get_value("t1_ovf"),
        "tank2" => crate::aog::sensors::get_value("t2_ovf"),
        _ => "NONE".to_string(),
    };
    
    if ovf_value.contains("OVERFLOW") {
        95.0
    } else {
        50.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mock_sensor() {
        let mut sensor = MockSensor::new(25.0);
        assert_eq!(sensor.read().unwrap(), 25.0);
        
        sensor.set_level(75.0);
        assert_eq!(sensor.read().unwrap(), 75.0);
    }
    
    #[test]
    fn test_water_level_monitor() {
        let sensor = Box::new(MockSensor::new(30.0));
        let config = WaterLevelConfig::default();
        let monitor = WaterLevelMonitor::new("test_tank".to_string(), sensor, config);
        
        let reading = monitor.get_level();
        assert_eq!(reading.tank_id, "test_tank");
        assert!(reading.is_valid);
        assert_eq!(reading.level_cm, 70.0); // 100cm tank - 30cm distance = 70cm water
        assert_eq!(reading.level_percent, 70.0);
    }
    
    #[test]
    fn test_moving_average() {
        let mut config = WaterLevelConfig::default();
        config.moving_average_samples = 3;
        
        let sensor = Box::new(MockSensor::new(30.0));
        let monitor = WaterLevelMonitor::new("test_tank".to_string(), sensor, config);
        
        // First reading
        let _ = monitor.get_level();
        
        // Change sensor value and get more readings
        if let Ok(mut sensor_guard) = monitor.sensor.lock() {
            if let Some(mock_sensor) = sensor_guard.as_any_mut().downcast_mut::<MockSensor>() {
                mock_sensor.set_level(40.0);
            }
        }
        
        // The moving average should smooth the transition
        let reading = monitor.get_level();
        assert!(reading.level_percent < 70.0 && reading.level_percent > 60.0);
    }
    
    #[test]
    fn test_calibration() {
        let sensor = Box::new(MockSensor::new(30.0));
        let config = WaterLevelConfig::default();
        let monitor = WaterLevelMonitor::new("test_tank".to_string(), sensor, config);
        
        assert!(monitor.calibrate(50.0).is_ok());
    }
    
    #[test]
    fn test_water_level_system() {
        let mut config = WaterLevelConfig::default();
        config.sensor_type = WaterLevelSensorType::Mock;
        
        let mut system = WaterLevelSystem::new(config);
        assert!(system.init().is_ok());
        
        let levels = system.get_all_levels();
        assert!(!levels.is_empty());
    }
}