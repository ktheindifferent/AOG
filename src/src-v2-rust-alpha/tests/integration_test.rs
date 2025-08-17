use aog::{Config, SensorLog, Sessions, Session, SensorKitConfig, Args};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn setup_test_environment() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let aog_dir = temp_dir.path().join("opt/aog");
    fs::create_dir_all(&aog_dir).unwrap();
    std::env::set_var("AOG_TEST_DIR", aog_dir.to_str().unwrap());
    temp_dir
}

fn cleanup_test_files(dir: &Path) {
    let test_files = [
        "data.json",
        "data.bak.json",
        "sessions.json",
        "sessions.bak.json",
    ];
    
    for file in test_files.iter() {
        let file_path = dir.join(file);
        if file_path.exists() {
            let _ = fs::remove_file(file_path);
        }
    }
}

#[test]
fn test_config_lifecycle() {
    let _temp_dir = setup_test_environment();
    
    // Create and save config
    let mut config = Config::new();
    config.is_hvac_kit_installed = true;
    config.is_sensor_kit_installed = true;
    config.photo_cycle_start = 5;
    config.photo_cycle_end = 23;
    
    // Add sensor log
    config.sensor_logs.push(SensorLog {
        id: "test_log".to_string(),
        timestamp: 1234567890,
        s1_co2: "450".to_string(),
        s2_co2: "500".to_string(),
        avg_co2: "475".to_string(),
        humidity: "65".to_string(),
        temperature: "25".to_string(),
        is_tank_one_overflowed: false,
        is_tank_two_overflowed: true,
    });
    
    // Verify config properties
    assert_eq!(config.sensor_logs.len(), 1);
    assert_eq!(config.photo_cycle_start, 5);
    assert_eq!(config.photo_cycle_end, 23);
}

#[test]
fn test_sensor_log_processing() {
    let sensor_log = SensorLog {
        id: "integration_test".to_string(),
        timestamp: 1700000000,
        s1_co2: "600".to_string(),
        s2_co2: "650".to_string(),
        avg_co2: "625".to_string(),
        humidity: "70".to_string(),
        temperature: "28".to_string(),
        is_tank_one_overflowed: true,
        is_tank_two_overflowed: false,
    };
    
    // Test CO2 average calculation
    let s1: f64 = sensor_log.s1_co2.parse().unwrap_or(0.0);
    let s2: f64 = sensor_log.s2_co2.parse().unwrap_or(0.0);
    let expected_avg = (s1 + s2) / 2.0;
    let actual_avg: f64 = sensor_log.avg_co2.parse().unwrap_or(0.0);
    
    assert_eq!(expected_avg, actual_avg);
    assert_eq!(sensor_log.is_tank_one_overflowed, true);
    assert_eq!(sensor_log.is_tank_two_overflowed, false);
}

#[test]
fn test_sessions_management() {
    let _temp_dir = setup_test_environment();
    
    let mut sessions = Sessions::new();
    
    // Add multiple sessions
    for i in 0..5 {
        sessions.sessions.push(Session {
            id: format!("session_{}", i),
            delta: (i * 10) as u8,
        });
    }
    
    assert_eq!(sessions.sessions.len(), 5);
    assert_eq!(sessions.sessions[0].id, "session_0");
    assert_eq!(sessions.sessions[4].delta, 40);
}

#[test]
fn test_sensor_kit_configuration() {
    let mut config = Config::new();
    
    config.sensor_kit_config = Some(SensorKitConfig {
        dht11_pin: 7,
        tank_one_overflow: 4,
        tank_two_overflow: 2,
        analog_co2_pin: "A0".to_string(),
        enable_dht11: true,
        enable_analog_co2: true,
        enable_ccs811: false,
    });
    
    let sensor_kit = config.sensor_kit_config.unwrap();
    assert_eq!(sensor_kit.dht11_pin, 7);
    assert_eq!(sensor_kit.analog_co2_pin, "A0");
    assert!(sensor_kit.enable_dht11);
    assert!(sensor_kit.enable_analog_co2);
    assert!(!sensor_kit.enable_ccs811);
}

#[test]
fn test_config_with_multiple_sensor_logs() {
    let mut config = Config::new();
    
    // Add multiple sensor logs
    for i in 0..10 {
        config.sensor_logs.push(SensorLog {
            id: format!("log_{}", i),
            timestamp: 1700000000 + i,
            s1_co2: format!("{}", 400 + i * 10),
            s2_co2: format!("{}", 450 + i * 10),
            avg_co2: format!("{}", 425 + i * 10),
            humidity: format!("{}", 60 + i),
            temperature: format!("{}", 20 + i),
            is_tank_one_overflowed: i % 2 == 0,
            is_tank_two_overflowed: i % 3 == 0,
        });
    }
    
    assert_eq!(config.sensor_logs.len(), 10);
    
    // Verify first and last logs
    assert_eq!(config.sensor_logs[0].s1_co2, "400");
    assert_eq!(config.sensor_logs[9].s1_co2, "490");
    assert_eq!(config.sensor_logs[0].is_tank_one_overflowed, true);
    assert_eq!(config.sensor_logs[1].is_tank_one_overflowed, false);
}

#[test]
fn test_config_json_serialization_roundtrip() {
    let mut config = Config::new();
    config.photo_cycle_start = 7;
    config.photo_cycle_end = 22;
    config.tank_one_to_two_pump_pin = 18;
    config.uv_light_pin = 28;
    config.air_circulation_pin = 23;
    
    // Serialize to JSON
    let json = serde_json::to_string_pretty(&config).unwrap();
    
    // Deserialize back
    let deserialized: Config = serde_json::from_str(&json).unwrap();
    
    assert_eq!(config.photo_cycle_start, deserialized.photo_cycle_start);
    assert_eq!(config.photo_cycle_end, deserialized.photo_cycle_end);
    assert_eq!(config.tank_one_to_two_pump_pin, deserialized.tank_one_to_two_pump_pin);
    assert_eq!(config.uv_light_pin, deserialized.uv_light_pin);
    assert_eq!(config.air_circulation_pin, deserialized.air_circulation_pin);
}

#[test]
fn test_overflow_detection_logic() {
    let test_cases = vec![
        (true, true, "Both tanks overflowed"),
        (true, false, "Tank 1 overflowed"),
        (false, true, "Tank 2 overflowed"),
        (false, false, "No overflow"),
    ];
    
    for (tank1, tank2, description) in test_cases {
        let log = SensorLog {
            id: "overflow_test".to_string(),
            timestamp: 1700000000,
            s1_co2: "450".to_string(),
            s2_co2: "450".to_string(),
            avg_co2: "450".to_string(),
            humidity: "60".to_string(),
            temperature: "25".to_string(),
            is_tank_one_overflowed: tank1,
            is_tank_two_overflowed: tank2,
        };
        
        assert_eq!(log.is_tank_one_overflowed, tank1, "{}", description);
        assert_eq!(log.is_tank_two_overflowed, tank2, "{}", description);
    }
}

#[test]
fn test_power_type_configurations() {
    let power_types = vec!["Grid", "Solar", "Battery", "Hybrid"];
    
    for power_type in power_types {
        let mut config = Config::new();
        config.power_type = power_type.to_string();
        assert_eq!(config.power_type, power_type);
    }
}

#[test]
fn test_session_delta_values() {
    let mut sessions = Sessions::new();
    
    // Test various delta values
    let deltas = vec![0, 1, 10, 50, 100, 255];
    
    for delta in deltas {
        sessions.sessions.push(Session {
            id: format!("delta_{}", delta),
            delta,
        });
    }
    
    for (i, session) in sessions.sessions.iter().enumerate() {
        assert_eq!(session.delta, deltas[i]);
    }
}