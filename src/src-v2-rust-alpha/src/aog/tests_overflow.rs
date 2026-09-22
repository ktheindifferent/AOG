// Copyright (c) 2024 - Water Overflow Safety Tests
// MIT License

use super::sensors::set_overflow_failure;
use std::fs;
use tempfile::TempDir;

#[test]
fn failure_blocks_both_tanks_and_records_the_reason() {
    let directory = TempDir::new().unwrap();
    fs::write(directory.path().join("t1_ovf"), "NONE").unwrap();
    fs::write(directory.path().join("t2_ovf"), "NONE").unwrap();

    set_overflow_failure(directory.path(), "disconnected at /dev/ttyUSB0").unwrap();

    for tank in ["t1_ovf", "t2_ovf"] {
        assert_eq!(
            fs::read_to_string(directory.path().join(tank)).unwrap(),
            "OVERFLOW"
        );
    }
    let error = fs::read_to_string(directory.path().join("overflow_error")).unwrap();
    assert!(error.starts_with("SENSOR_FAILURE:"));
    assert!(error.contains("disconnected at /dev/ttyUSB0"));
    assert!(error.contains("timestamp:"));
}

#[test]
fn failure_creates_a_missing_sensor_directory() {
    let directory = TempDir::new().unwrap();
    let sensors = directory.path().join("sensors");
    set_overflow_failure(&sensors, "initializing").unwrap();
    assert_eq!(
        fs::read_to_string(sensors.join("t1_ovf")).unwrap(),
        "OVERFLOW"
    );
    assert!(sensors.join("overflow_error").exists());
}

#[test]
fn interlock_and_other_tank_are_written_if_one_tank_cannot_be_updated() {
    let directory = TempDir::new().unwrap();
    fs::create_dir(directory.path().join("t1_ovf")).unwrap();
    assert!(set_overflow_failure(directory.path(), "read failure").is_err());
    assert!(directory.path().join("overflow_error").exists());
    assert_eq!(
        fs::read_to_string(directory.path().join("t2_ovf")).unwrap(),
        "OVERFLOW"
    );
}
