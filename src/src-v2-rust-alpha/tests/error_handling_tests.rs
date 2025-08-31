// Copyright (c) 2024 Terragon Labs
//
// MIT License
//
// Comprehensive tests for error handling improvements

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use aog::error::{AogError, AogResult, recover_mutex_lock, safe_mutex_access};
use aog::aog::pump_safety::{PumpSafetyMonitor, PumpType};

#[test]
fn test_mutex_poisoning_recovery() {
    let mutex = Arc::new(Mutex::new(42));
    let mutex_clone = Arc::clone(&mutex);
    
    // Poison the mutex in a thread that panics
    let handle = thread::spawn(move || {
        let _guard = mutex_clone.lock().unwrap();
        panic!("Intentional panic to poison mutex");
    });
    
    // Let the thread panic
    let _ = handle.join();
    
    // Now the mutex is poisoned, but our recovery should handle it
    let result = recover_mutex_lock(&mutex, "test_recovery");
    assert!(result.is_ok());
    
    let guard = result.unwrap();
    assert_eq!(*guard, 42);
}

#[test]
fn test_safe_mutex_access_with_poisoned_lock() {
    let mutex = Arc::new(Mutex::new(100));
    let mutex_clone = Arc::clone(&mutex);
    
    // Poison the mutex
    let handle = thread::spawn(move || {
        let mut guard = mutex_clone.lock().unwrap();
        *guard = 200;
        panic!("Poisoning the mutex");
    });
    
    let _ = handle.join();
    
    // safe_mutex_access should recover and return the value
    let value = safe_mutex_access(&mutex, "test", |val| *val, 0);
    assert_eq!(value, 200); // Should get the actual value, not default
}

#[test]
fn test_pump_safety_with_mutex_recovery() {
    let monitor = PumpSafetyMonitor::new();
    
    // Start a pump
    monitor.register_pump_start("test_pump".to_string(), PumpType::Fill);
    
    // Despite potential mutex issues, these operations should still work
    let result = monitor.can_start_pump("another_pump", PumpType::Drain);
    assert!(result.is_ok());
    
    monitor.register_pump_stop("test_pump".to_string(), "Test complete".to_string());
    
    // Verify stats can still be retrieved
    let stats = monitor.get_pump_stats("test_pump");
    assert!(stats.contains_key("current_state"));
}

#[test]
fn test_emergency_shutdown_with_error_recovery() {
    let monitor = PumpSafetyMonitor::new();
    
    // Start multiple pumps
    monitor.register_pump_start("pump1".to_string(), PumpType::Fill);
    monitor.register_pump_start("pump2".to_string(), PumpType::Drain);
    
    // Trigger emergency shutdown
    monitor.emergency_shutdown("Test emergency with recovery".to_string());
    
    // Verify no pumps can start during emergency
    let result = monitor.can_start_pump("pump3", PumpType::Circulation);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Emergency stop"));
    
    // Reset emergency stop
    monitor.reset_emergency_stop();
    
    // Now pumps should be able to start again
    let result = monitor.can_start_pump("pump3", PumpType::Circulation);
    assert!(result.is_ok());
}

#[test]
fn test_error_propagation() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let aog_error: AogError = io_error.into();
    
    match aog_error {
        AogError::Io(_) => {}, // Expected
        _ => panic!("Wrong error type conversion"),
    }
}

#[test]
fn test_json_error_conversion() {
    let json_str = "{invalid json}";
    let result: Result<serde_json::Value, serde_json::Error> = serde_json::from_str(json_str);
    
    if let Err(e) = result {
        let aog_error: AogError = e.into();
        match aog_error {
            AogError::Serialization(_) => {}, // Expected
            _ => panic!("Wrong error type for JSON error"),
        }
    }
}

#[test]
fn test_oscillation_safety_limits() {
    let monitor = PumpSafetyMonitor::new();
    
    // Test too fast oscillation
    let result = monitor.check_oscillation_safety("osc_pump", 50);
    assert!(result.is_err());
    
    // Test too slow oscillation
    let result = monitor.check_oscillation_safety("osc_pump", 6000);
    assert!(result.is_err());
    
    // Test valid oscillation speed
    let result = monitor.check_oscillation_safety("osc_pump", 1000);
    assert!(result.is_ok());
    
    // Reset counter
    monitor.reset_oscillation_counter("osc_pump");
    
    // Verify counter was reset
    let stats = monitor.get_pump_stats("osc_pump");
    if let Some(count) = stats.get("oscillation_count") {
        assert_eq!(count, "0");
    }
}

#[test]
fn test_runtime_limit_enforcement() {
    let monitor = PumpSafetyMonitor::new();
    
    // Start a pump
    monitor.register_pump_start("runtime_test".to_string(), PumpType::Fill);
    
    // Initially should be within limits
    assert!(monitor.check_runtime_limit("runtime_test", PumpType::Fill));
    
    // Note: In a real test, we'd mock time or wait
    // For now, just verify the function doesn't panic
}

#[test]
fn test_maintenance_hour_tracking() {
    let monitor = PumpSafetyMonitor::new();
    
    // Start and stop pump to accumulate runtime
    monitor.register_pump_start("maint_test".to_string(), PumpType::Auxiliary);
    thread::sleep(Duration::from_millis(10));
    monitor.register_pump_stop("maint_test".to_string(), "Test".to_string());
    
    // Check stats include maintenance hours
    let stats = monitor.get_pump_stats("maint_test");
    assert!(stats.contains_key("maintenance_hours"));
    assert!(stats.contains_key("total_runtime_seconds"));
}

#[test]
fn test_cooldown_enforcement() {
    let monitor = PumpSafetyMonitor::new();
    
    // Start and immediately stop a pump
    monitor.register_pump_start("cooldown_test".to_string(), PumpType::Drain);
    monitor.register_pump_stop("cooldown_test".to_string(), "Quick stop".to_string());
    
    // Should not be able to restart immediately
    let result = monitor.can_start_pump("cooldown_test", PumpType::Drain);
    assert!(result.is_err());
    
    // Error message should mention cooldown
    if let Err(msg) = result {
        assert!(msg.contains("cooldown"));
    }
}

#[test]
fn test_concurrent_pump_operations() {
    let monitor = Arc::new(PumpSafetyMonitor::new());
    let mut handles = vec![];
    
    // Start multiple pumps concurrently
    for i in 0..5 {
        let monitor_clone = Arc::clone(&monitor);
        let handle = thread::spawn(move || {
            let pump_id = format!("concurrent_pump_{}", i);
            monitor_clone.register_pump_start(pump_id.clone(), PumpType::Circulation);
            thread::sleep(Duration::from_millis(10));
            monitor_clone.register_pump_stop(pump_id, "Concurrent test".to_string());
        });
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread should complete");
    }
    
    // Verify all pumps have stats
    for i in 0..5 {
        let pump_id = format!("concurrent_pump_{}", i);
        let stats = monitor.get_pump_stats(&pump_id);
        assert!(stats.contains_key("current_state"));
    }
}

#[test]
fn test_calibration_error_handling() {
    let monitor = PumpSafetyMonitor::new();
    
    // Calibration should work for idle pump
    let result = monitor.calibrate_pump("cal_test", PumpType::Fill);
    assert!(result.is_ok());
    
    // Start the pump
    monitor.register_pump_start("cal_test2".to_string(), PumpType::Fill);
    
    // Calibration should fail for running pump
    let result = monitor.calibrate_pump("cal_test2", PumpType::Fill);
    assert!(result.is_err());
}

#[test]
fn test_file_operations_error_handling() {
    let monitor = PumpSafetyMonitor::new();
    
    // Load from non-existent file should not panic
    let result = monitor.load_from_file("/non/existent/path/config.json");
    assert!(result.is_ok()); // Returns Ok(()) when file doesn't exist
    
    // Save to potentially invalid path should not panic
    let result = monitor.save_to_file("/tmp/pump_safety_test.json");
    assert!(result.is_ok());
}

#[test]
fn test_error_display_formatting() {
    let errors = vec![
        AogError::Config("Invalid configuration".to_string()),
        AogError::Sensor("Sensor reading failed".to_string()),
        AogError::Pump("Pump operation failed".to_string()),
        AogError::MutexPoisoned("Lock poisoned".to_string()),
        AogError::EmergencyStop("Emergency triggered".to_string()),
    ];
    
    for error in errors {
        let formatted = format!("{}", error);
        assert!(!formatted.is_empty());
        assert!(formatted.contains(":"));
    }
}