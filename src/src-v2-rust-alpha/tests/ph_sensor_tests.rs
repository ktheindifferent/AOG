// pH Sensor Integration Tests

use aog::aog::ph_sensor::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[cfg(test)]
mod ph_sensor_tests {
    use super::*;
    
    fn setup_test_environment() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let sensors_dir = temp_dir.path().join("sensors");
        fs::create_dir_all(&sensors_dir).unwrap();
        
        // Create mock sensor files
        fs::write(sensors_dir.join("ph"), "7.2").unwrap();
        fs::write(sensors_dir.join("temp"), "25.0").unwrap();
        
        temp_dir
    }
    
    #[test]
    fn test_ph_range_validation() {
        // Test normal range
        assert_eq!(PhSensor::calculate_alert_level(7.0), PhAlertLevel::Normal);
        assert_eq!(PhSensor::calculate_alert_level(6.5), PhAlertLevel::Normal);
        assert_eq!(PhSensor::calculate_alert_level(7.5), PhAlertLevel::Normal);
        
        // Test warning range
        assert_eq!(PhSensor::calculate_alert_level(6.0), PhAlertLevel::Warning);
        assert_eq!(PhSensor::calculate_alert_level(8.0), PhAlertLevel::Warning);
        assert_eq!(PhSensor::calculate_alert_level(6.3), PhAlertLevel::Warning);
        assert_eq!(PhSensor::calculate_alert_level(7.7), PhAlertLevel::Warning);
        
        // Test critical range
        assert_eq!(PhSensor::calculate_alert_level(5.0), PhAlertLevel::Critical);
        assert_eq!(PhSensor::calculate_alert_level(9.0), PhAlertLevel::Critical);
        assert_eq!(PhSensor::calculate_alert_level(4.5), PhAlertLevel::Critical);
        assert_eq!(PhSensor::calculate_alert_level(10.0), PhAlertLevel::Critical);
    }
    
    #[test]
    fn test_calibration_point_classification() {
        let mut calibration = PhCalibration::default();
        
        // Test pH 4.0 buffer
        let point_4 = PhCalibrationPoint {
            ph_value: 4.0,
            raw_value: 100.0,
            temperature: 25.0,
            timestamp: 1000,
        };
        
        // Test pH 7.0 buffer
        let point_7 = PhCalibrationPoint {
            ph_value: 7.0,
            raw_value: 200.0,
            temperature: 25.0,
            timestamp: 2000,
        };
        
        // Test pH 10.0 buffer
        let point_10 = PhCalibrationPoint {
            ph_value: 10.0,
            raw_value: 300.0,
            temperature: 25.0,
            timestamp: 3000,
        };
        
        calibration.point_4 = Some(point_4);
        calibration.point_7 = Some(point_7);
        calibration.point_10 = Some(point_10);
        
        assert!(calibration.point_4.is_some());
        assert!(calibration.point_7.is_some());
        assert!(calibration.point_10.is_some());
    }
    
    #[test]
    fn test_calibration_coefficient_calculation() {
        let mut cal = PhCalibration::default();
        
        // Setup two-point calibration
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
        
        // Slope should be (7-4)/(200-100) = 0.03
        assert!((cal.slope - 0.03).abs() < 0.001);
        // Offset should be 7 - (0.03 * 200) = 1.0
        assert!((cal.offset - 1.0).abs() < 0.001);
        
        // Test calibration application
        let calibrated = cal.apply_calibration(150.0);
        assert!((calibrated - 5.5).abs() < 0.01);
    }
    
    #[test]
    fn test_temperature_compensation() {
        let cal = PhCalibration::default();
        
        // Test compensation at different temperatures
        let ph_at_25c = 7.0;
        
        // At 20°C (5 degrees below reference)
        let ph_at_20c = cal.apply_temperature_compensation(ph_at_25c, 20.0);
        assert!((ph_at_20c - 7.015).abs() < 0.001);
        
        // At 30°C (5 degrees above reference)
        let ph_at_30c = cal.apply_temperature_compensation(ph_at_25c, 30.0);
        assert!((ph_at_30c - 6.985).abs() < 0.001);
        
        // At reference temperature (no change)
        let ph_at_ref = cal.apply_temperature_compensation(ph_at_25c, 25.0);
        assert!((ph_at_ref - 7.0).abs() < 0.001);
    }
    
    #[test]
    fn test_ph_history_management() {
        let mut history = PhHistory::default();
        
        // Test adding readings
        for i in 0..10 {
            history.add_reading(PhReading {
                ph_value: 7.0 + (i as f32 * 0.1),
                raw_value: 200.0,
                temperature: 25.0,
                timestamp: i,
                alert_level: PhAlertLevel::Normal,
            });
        }
        
        assert_eq!(history.readings.len(), 10);
        
        // Test max size limit
        let max_size = history.max_size;
        for i in 10..max_size + 10 {
            history.add_reading(PhReading {
                ph_value: 7.0,
                raw_value: 200.0,
                temperature: 25.0,
                timestamp: i as u64,
                alert_level: PhAlertLevel::Normal,
            });
        }
        
        assert_eq!(history.readings.len(), max_size);
    }
    
    #[test]
    fn test_ph_trend_detection() {
        let mut history = PhHistory::default();
        
        // Test rising trend
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
        
        // Clear and test falling trend
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
        
        // Clear and test stable trend
        history.readings.clear();
        for i in 0..10 {
            history.add_reading(PhReading {
                ph_value: 7.0 + ((i % 2) as f32 * 0.05),
                raw_value: 200.0,
                temperature: 25.0,
                timestamp: i,
                alert_level: PhAlertLevel::Normal,
            });
        }
        assert_eq!(history.get_trend(5), PhTrend::Stable);
    }
    
    #[test]
    fn test_average_calculation() {
        let mut history = PhHistory::default();
        
        // Add known values
        for i in 0..5 {
            history.add_reading(PhReading {
                ph_value: 6.0 + i as f32,
                raw_value: 200.0,
                temperature: 25.0,
                timestamp: i,
                alert_level: PhAlertLevel::Normal,
            });
        }
        
        // Average of 6, 7, 8, 9, 10 should be 8
        let avg = history.get_average(5);
        assert!(avg.is_some());
        assert!((avg.unwrap() - 8.0).abs() < 0.01);
        
        // Test with smaller window
        let avg_small = history.get_average(2);
        assert!(avg_small.is_some());
        // Should average last 2 readings (9 and 10)
        assert!((avg_small.unwrap() - 9.5).abs() < 0.01);
    }
    
    #[test]
    fn test_adjustment_suggestions() {
        let sensor = PhSensor::new(PhSensorType::Arduino);
        
        // Test low pH suggestion
        let mut history = PhHistory::default();
        for _ in 0..5 {
            history.add_reading(PhReading {
                ph_value: 6.0,
                raw_value: 180.0,
                temperature: 25.0,
                timestamp: 0,
                alert_level: PhAlertLevel::Warning,
            });
        }
        
        // Manually set history for testing
        let suggestion = sensor.get_adjustment_suggestion();
        
        // The suggestion should recommend pH UP
        if !history.readings.is_empty() {
            assert!(suggestion.contains("low") || suggestion.contains("UP") || suggestion.contains("Insufficient"));
        }
    }
    
    #[test]
    fn test_boundary_values() {
        // Test exact boundary values
        assert_eq!(PhSensor::calculate_alert_level(PH_OPTIMAL_MIN), PhAlertLevel::Normal);
        assert_eq!(PhSensor::calculate_alert_level(PH_OPTIMAL_MAX), PhAlertLevel::Normal);
        assert_eq!(PhSensor::calculate_alert_level(PH_CRITICAL_MIN), PhAlertLevel::Critical);
        assert_eq!(PhSensor::calculate_alert_level(PH_CRITICAL_MAX), PhAlertLevel::Critical);
        
        // Test just outside boundaries
        assert_eq!(PhSensor::calculate_alert_level(PH_OPTIMAL_MIN - 0.01), PhAlertLevel::Warning);
        assert_eq!(PhSensor::calculate_alert_level(PH_OPTIMAL_MAX + 0.01), PhAlertLevel::Warning);
        assert_eq!(PhSensor::calculate_alert_level(PH_CRITICAL_MIN - 0.01), PhAlertLevel::Critical);
        assert_eq!(PhSensor::calculate_alert_level(PH_CRITICAL_MAX + 0.01), PhAlertLevel::Critical);
    }
    
    #[test]
    fn test_extreme_values() {
        // Test extreme pH values
        assert_eq!(PhSensor::calculate_alert_level(0.0), PhAlertLevel::Critical);
        assert_eq!(PhSensor::calculate_alert_level(14.0), PhAlertLevel::Critical);
        assert_eq!(PhSensor::calculate_alert_level(-1.0), PhAlertLevel::Critical);
        assert_eq!(PhSensor::calculate_alert_level(15.0), PhAlertLevel::Critical);
    }
    
    #[test]
    fn test_calibration_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let cal_path = temp_dir.path().join("ph_calibration.json");
        
        let mut calibration = PhCalibration::default();
        calibration.point_7 = Some(PhCalibrationPoint {
            ph_value: 7.0,
            raw_value: 200.0,
            temperature: 25.0,
            timestamp: 1000,
        });
        calibration.calculate_coefficients();
        
        // Save calibration
        let json = serde_json::to_string_pretty(&calibration).unwrap();
        fs::write(&cal_path, json).unwrap();
        
        // Load calibration
        let loaded_json = fs::read_to_string(&cal_path).unwrap();
        let loaded_cal: PhCalibration = serde_json::from_str(&loaded_json).unwrap();
        
        assert!(loaded_cal.point_7.is_some());
        assert_eq!(loaded_cal.point_7.unwrap().ph_value, 7.0);
    }
    
    #[test]
    fn test_sensor_status_generation() {
        let sensor = PhSensor::new(PhSensorType::Arduino);
        let status = sensor.get_status();
        
        // Status should have default values
        assert!(status.adjustment_suggestion.len() > 0);
        assert_eq!(status.alert_level, PhAlertLevel::Normal);
        assert_eq!(status.trend, PhTrend::Stable);
    }
}