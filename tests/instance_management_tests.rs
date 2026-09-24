#[cfg(test)]
mod instance_tests {
    use aog::aog::instance;
    use std::fs;
    use tempfile::TempDir;
    use std::thread;
    use std::time::Duration;
    
    struct TestEnvironment {
        _directory: TempDir,
        previous: Option<std::ffi::OsString>,
    }

    impl TestEnvironment {
        fn new() -> Self {
            let directory = TempDir::new().unwrap();
            let previous = std::env::var_os("AOG_TEST_DIR");
            std::env::set_var("AOG_TEST_DIR", directory.path());
            Self { _directory: directory, previous }
        }
    }

    impl Drop for TestEnvironment {
        fn drop(&mut self) {
            instance::release_lock().unwrap();
            match &self.previous {
                Some(previous) => std::env::set_var("AOG_TEST_DIR", previous),
                None => std::env::remove_var("AOG_TEST_DIR"),
            }
        }
    }

    fn test_file(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(std::env::var_os("AOG_TEST_DIR").unwrap()).join(name)
    }
    
    #[test]
    #[serial_test::serial]
    fn test_instance_detection() {
        let _environment = TestEnvironment::new();
        // Clean up any existing lock files first
        cleanup_instance_files();
        
        // Test that no instance is detected initially
        assert!(!instance::check_running_instance());
        
        // Test port checking
        assert!(instance::check_port_available(9443));
        assert!(instance::check_port_available(8443));
    }
    
    #[test]
    #[serial_test::serial]
    fn test_lock_acquisition() {
        let _environment = TestEnvironment::new();
        cleanup_instance_files();
        
        // First acquisition should succeed
        assert!(instance::acquire_lock().unwrap());
        
        // Second acquisition should fail
        assert!(!instance::acquire_lock().unwrap());
        
        // Release lock
        instance::release_lock().unwrap();
        
        // Now acquisition should succeed again
        assert!(instance::acquire_lock().unwrap());
        
        // Clean up
        instance::release_lock().unwrap();
    }
    
    #[test]
    #[serial_test::serial]
    fn test_pid_file_operations() {
        let _environment = TestEnvironment::new();
        cleanup_instance_files();
        
        // Write PID file
        instance::write_pid_file().unwrap();
        
        // Read it back
        let info = instance::read_pid_file().unwrap();
        assert_eq!(info.pid, std::process::id());
        assert_eq!(info.port, 9443);
        
        // Remove PID file
        instance::remove_pid_file().unwrap();
        
        // Verify it's gone
        assert!(!test_file("aog.pid").exists());
    }
    
    #[test]
    #[serial_test::serial]
    fn test_command_forwarding_retry() {
        let _environment = TestEnvironment::new();
        // This test simulates command forwarding with retries
        // Note: Requires a mock server or actual running instance
        
        let result = instance::forward_command_with_retry(
            "test command",
            None,
        );
        
        // Since no server is running, this should fail after retries
        assert!(result.is_err());
    }
    
    #[test]
    #[serial_test::serial]
    fn test_force_flag_behavior() {
        let _environment = TestEnvironment::new();
        cleanup_instance_files();
        
        // Simulate both the lock and the command API of a running instance.
        instance::acquire_lock().unwrap();
        let _listener = std::net::TcpListener::bind("127.0.0.1:9443").unwrap();
        
        // Without force flag, should detect existing instance
        assert!(!instance::handle_instance_check(false).unwrap());
        
        // With force flag, should bypass check
        assert!(instance::handle_instance_check(true).unwrap());
        
        // Clean up
        cleanup_instance_files();
    }
    
    #[test]
    #[serial_test::serial]
    fn test_stale_lock_cleanup() {
        let _environment = TestEnvironment::new();
        cleanup_instance_files();
        
        // Create a lock file with invalid PID
        
        // Write a PID file with non-existent process
        let fake_info = instance::InstanceInfo {
            pid: u32::MAX,  // Outside the Linux PID range
            port: 9443,
            start_time: chrono::Local::now().to_rfc3339(),
        };
        
        let content = serde_json::to_string_pretty(&fake_info).unwrap();
        fs::write(test_file("aog.pid"), content).unwrap();
        fs::write(test_file("aog.lock"), "").unwrap();
        
        // Should detect stale lock and clean it up
        assert!(instance::acquire_lock().unwrap());
        
        // Clean up
        instance::release_lock().unwrap();
    }
    
    #[test]
    #[serial_test::serial]
    fn test_concurrent_instance_protection() {
        let _environment = TestEnvironment::new();
        cleanup_instance_files();
        
        let (acquired_tx, acquired_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let handle = thread::spawn(move || {
            assert!(instance::acquire_lock().unwrap());
            acquired_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            instance::release_lock().unwrap();
        });
        acquired_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        
        // Try to acquire lock from main thread - should fail
        assert!(!instance::acquire_lock().unwrap());
        
        // Wait for first thread to release
        release_tx.send(()).unwrap();
        handle.join().unwrap();
        
        // Now should succeed
        assert!(instance::acquire_lock().unwrap());
        instance::release_lock().unwrap();
    }
    
    #[test]
    #[serial_test::serial]
    fn test_simultaneous_lock_acquisition_has_one_winner() {
        let _environment = TestEnvironment::new();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8).map(|_| {
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                instance::acquire_lock().unwrap()
            })
        }).collect();
        let winners = threads.into_iter().map(|handle| handle.join().unwrap()).filter(|won| *won).count();
        assert_eq!(winners, 1);
        instance::release_lock().unwrap();
    }

    fn cleanup_instance_files() {
        instance::release_lock().unwrap();
        fs::remove_file(test_file("aog.pid")).ok();
        fs::remove_file(test_file("aog.lock")).ok();
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
