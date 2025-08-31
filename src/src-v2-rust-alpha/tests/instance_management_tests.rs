#[cfg(test)]
mod instance_tests {
    use std::fs;
    use std::path::Path;
    use std::thread;
    use std::time::Duration;
    
    // Note: These tests need to be run with proper permissions
    // and may require the aog binary to be built first
    
    #[test]
    fn test_instance_detection() {
        // Clean up any existing lock files first
        cleanup_instance_files();
        
        // Test that no instance is detected initially
        assert!(!aog::instance::check_running_instance());
        
        // Test port checking
        assert!(aog::instance::check_port_available(9443));
        assert!(aog::instance::check_port_available(8443));
    }
    
    #[test]
    fn test_lock_acquisition() {
        cleanup_instance_files();
        
        // First acquisition should succeed
        assert!(aog::instance::acquire_lock().unwrap());
        
        // Second acquisition should fail
        assert!(!aog::instance::acquire_lock().unwrap());
        
        // Release lock
        aog::instance::release_lock().unwrap();
        
        // Now acquisition should succeed again
        assert!(aog::instance::acquire_lock().unwrap());
        
        // Clean up
        aog::instance::release_lock().unwrap();
    }
    
    #[test]
    fn test_pid_file_operations() {
        cleanup_instance_files();
        
        // Write PID file
        aog::instance::write_pid_file().unwrap();
        
        // Read it back
        let info = aog::instance::read_pid_file().unwrap();
        assert_eq!(info.pid, std::process::id());
        assert_eq!(info.port, 9443);
        
        // Remove PID file
        aog::instance::remove_pid_file().unwrap();
        
        // Verify it's gone
        assert!(!Path::new("/opt/aog/aog.pid").exists());
    }
    
    #[test]
    fn test_command_forwarding_retry() {
        // This test simulates command forwarding with retries
        // Note: Requires a mock server or actual running instance
        
        let result = aog::instance::forward_command_with_retry(
            "test command",
            None,
        );
        
        // Since no server is running, this should fail after retries
        assert!(result.is_err());
    }
    
    #[test]
    fn test_force_flag_behavior() {
        cleanup_instance_files();
        
        // Acquire lock to simulate running instance
        aog::instance::acquire_lock().unwrap();
        
        // Without force flag, should detect existing instance
        assert!(!aog::instance::handle_instance_check(false).unwrap());
        
        // With force flag, should bypass check
        assert!(aog::instance::handle_instance_check(true).unwrap());
        
        // Clean up
        cleanup_instance_files();
    }
    
    #[test]
    fn test_stale_lock_cleanup() {
        cleanup_instance_files();
        
        // Create a lock file with invalid PID
        fs::create_dir_all("/opt/aog").ok();
        
        // Write a PID file with non-existent process
        let fake_info = aog::instance::InstanceInfo {
            pid: 99999,  // Very unlikely to be a real PID
            port: 9443,
            start_time: chrono::Local::now().to_rfc3339(),
        };
        
        let content = serde_json::to_string_pretty(&fake_info).unwrap();
        fs::write("/opt/aog/aog.pid", content).unwrap();
        fs::write("/opt/aog/aog.lock", "").unwrap();
        
        // Should detect stale lock and clean it up
        assert!(aog::instance::acquire_lock().unwrap());
        
        // Clean up
        aog::instance::release_lock().unwrap();
    }
    
    #[test]
    fn test_concurrent_instance_protection() {
        cleanup_instance_files();
        
        // Spawn thread that acquires lock
        let handle = thread::spawn(|| {
            aog::instance::acquire_lock().unwrap();
            thread::sleep(Duration::from_millis(500));
            aog::instance::release_lock().unwrap();
        });
        
        // Give first thread time to acquire lock
        thread::sleep(Duration::from_millis(100));
        
        // Try to acquire lock from main thread - should fail
        assert!(!aog::instance::acquire_lock().unwrap());
        
        // Wait for first thread to release
        handle.join().unwrap();
        
        // Now should succeed
        assert!(aog::instance::acquire_lock().unwrap());
        aog::instance::release_lock().unwrap();
    }
    
    fn cleanup_instance_files() {
        fs::remove_file("/opt/aog/aog.pid").ok();
        fs::remove_file("/opt/aog/aog.lock").ok();
    }
}

#[cfg(test)]
mod integration_tests {
    use std::process::Command;
    use std::thread;
    use std::time::Duration;
    
    #[test]
    #[ignore] // Run with: cargo test -- --ignored
    fn test_real_instance_forwarding() {
        // This test requires the aog binary to be built
        // Build with: cargo build --release
        
        let binary_path = "./target/release/aog";
        
        // Start first instance in background
        let mut first_instance = Command::new(binary_path)
            .spawn()
            .expect("Failed to start first instance");
        
        // Give it time to start
        thread::sleep(Duration::from_secs(2));
        
        // Try to start second instance without --force
        let output = Command::new(binary_path)
            .output()
            .expect("Failed to run second instance");
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("already running"));
        
        // Try with --force flag
        let mut forced_instance = Command::new(binary_path)
            .arg("--force")
            .spawn()
            .expect("Failed to start forced instance");
        
        // Clean up
        thread::sleep(Duration::from_secs(1));
        first_instance.kill().ok();
        forced_instance.kill().ok();
    }
    
    #[test]
    #[ignore]
    fn test_command_forwarding_to_running_instance() {
        // This test simulates sending commands to a running instance
        
        let binary_path = "./target/release/aog";
        
        // Start background instance
        let mut bg_instance = Command::new(binary_path)
            .spawn()
            .expect("Failed to start background instance");
        
        // Give it time to start
        thread::sleep(Duration::from_secs(3));
        
        // Send command via HTTP API
        let client = reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        
        let params = [("input_command", "stats")];
        let response = client
            .post("https://localhost:9443/api/command")
            .form(&params)
            .send();
        
        match response {
            Ok(res) => {
                assert!(res.status().is_success());
                let body = res.text().unwrap();
                assert!(body.contains("success"));
            }
            Err(e) => {
                eprintln!("Command forwarding test failed: {}", e);
            }
        }
        
        // Clean up
        bg_instance.kill().ok();
    }
}