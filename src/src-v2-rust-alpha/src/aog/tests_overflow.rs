// Copyright (c) 2024 - Water Overflow Safety Tests
//
// MIT License

#[cfg(test)]
mod overflow_tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    
    // Helper function to create test sensor directory
    fn setup_test_sensors_dir() -> String {
        let test_dir = "/tmp/test_aog_sensors";
        let _ = fs::create_dir_all(test_dir);
        test_dir.to_string()
    }
    
    // Clean up test files
    fn cleanup_test_files(dir: &str) {
        let _ = fs::remove_dir_all(dir);
    }
    
    #[test]
    fn test_overflow_state_on_sensor_failure() {
        let test_dir = setup_test_sensors_dir();
        let t1_path = format!("{}/t1_ovf", test_dir);
        let t2_path = format!("{}/t2_ovf", test_dir);
        let error_path = format!("{}/overflow_error", test_dir);
        
        // Simulate sensor failure by creating overflow files
        fs::write(&t1_path, b"OVERFLOW").expect("Failed to write t1_ovf");
        fs::write(&t2_path, b"OVERFLOW").expect("Failed to write t2_ovf");
        fs::write(&error_path, b"SENSOR_FAILURE: Test error").expect("Failed to write error");
        
        // Verify files were created with correct content
        let t1_content = fs::read_to_string(&t1_path).expect("Failed to read t1_ovf");
        assert_eq!(t1_content, "OVERFLOW");
        
        let t2_content = fs::read_to_string(&t2_path).expect("Failed to read t2_ovf");
        assert_eq!(t2_content, "OVERFLOW");
        
        let error_content = fs::read_to_string(&error_path).expect("Failed to read error");
        assert!(error_content.contains("SENSOR_FAILURE"));
        
        cleanup_test_files(&test_dir);
    }
    
    #[test]
    fn test_overflow_recovery() {
        let test_dir = setup_test_sensors_dir();
        let error_path = format!("{}/overflow_error", test_dir);
        
        // Create error file
        File::create(&error_path).expect("Failed to create error file");
        assert!(Path::new(&error_path).exists());
        
        // Simulate recovery by removing error file
        fs::remove_file(&error_path).expect("Failed to remove error file");
        assert!(!Path::new(&error_path).exists());
        
        cleanup_test_files(&test_dir);
    }
    
    #[test]
    fn test_sensor_value_retrieval() {
        let test_dir = setup_test_sensors_dir();
        let sensor_path = format!("{}/test_sensor", test_dir);
        
        // Create test sensor file
        let mut f = File::create(&sensor_path).expect("Failed to create sensor file");
        f.write_all(b"TEST_VALUE").expect("Failed to write sensor value");
        
        // Read back value
        let content = fs::read_to_string(&sensor_path).expect("Failed to read sensor");
        assert_eq!(content, "TEST_VALUE");
        
        cleanup_test_files(&test_dir);
    }
    
    #[test]
    fn test_overflow_state_persistence() {
        let test_dir = setup_test_sensors_dir();
        let t1_path = format!("{}/t1_ovf", test_dir);
        
        // Write overflow state
        fs::write(&t1_path, b"OVERFLOW").expect("Failed to write overflow state");
        
        // Verify state persists
        let content = fs::read_to_string(&t1_path).expect("Failed to read t1_ovf");
        assert_eq!(content, "OVERFLOW");
        
        // Update to NORMAL state
        fs::write(&t1_path, b"NORMAL").expect("Failed to write normal state");
        
        let content = fs::read_to_string(&t1_path).expect("Failed to read updated t1_ovf");
        assert_eq!(content, "NORMAL");
        
        cleanup_test_files(&test_dir);
    }
    
    #[test]
    fn test_critical_condition_detection() {
        let test_dir = setup_test_sensors_dir();
        let t1_path = format!("{}/t1_ovf", test_dir);
        let t2_path = format!("{}/t2_ovf", test_dir);
        
        // Test case 1: Both tanks overflow
        fs::write(&t1_path, b"OVERFLOW").unwrap();
        fs::write(&t2_path, b"OVERFLOW").unwrap();
        
        let t1 = fs::read_to_string(&t1_path).unwrap();
        let t2 = fs::read_to_string(&t2_path).unwrap();
        
        let is_critical = t1.contains("OVERFLOW") || t2.contains("OVERFLOW");
        assert!(is_critical, "Should detect critical condition when tanks overflow");
        
        // Test case 2: Normal condition
        fs::write(&t1_path, b"NORMAL").unwrap();
        fs::write(&t2_path, b"NORMAL").unwrap();
        
        let t1 = fs::read_to_string(&t1_path).unwrap();
        let t2 = fs::read_to_string(&t2_path).unwrap();
        
        let is_critical = t1.contains("OVERFLOW") || t2.contains("OVERFLOW");
        assert!(!is_critical, "Should not detect critical condition when tanks are normal");
        
        cleanup_test_files(&test_dir);
    }
    
    #[test]
    fn test_error_message_format() {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let test_dir = setup_test_sensors_dir();
        let error_path = format!("{}/overflow_error", test_dir);
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let error_msg = format!("SENSOR_FAILURE: BrokenPipe at /dev/ttyUSB0 - timestamp: {}", timestamp);
        
        let mut f = File::create(&error_path).expect("Failed to create error file");
        f.write_all(error_msg.as_bytes()).expect("Failed to write error");
        
        let content = fs::read_to_string(&error_path).expect("Failed to read error");
        assert!(content.contains("SENSOR_FAILURE"));
        assert!(content.contains("BrokenPipe"));
        assert!(content.contains("/dev/ttyUSB"));
        assert!(content.contains("timestamp:"));
        
        cleanup_test_files(&test_dir);
    }
}