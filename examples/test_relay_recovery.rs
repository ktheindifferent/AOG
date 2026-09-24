use aog::{QwiicRelayDevice, RecoveryConfig};

fn main() {
    // Initialize logging
    simple_logger::SimpleLogger::new().init().unwrap();
    
    println!("Testing Qwiic Relay Recovery Mechanism");
    println!("========================================");
    
    // Test 1: Create device with default recovery config
    println!("\n1. Creating device with default recovery config...");
    let device = QwiicRelayDevice::new(0x25);
    println!("   Device created with ID: 0x{:02X}", device.id);
    
    // Test 2: Create device with custom recovery config
    println!("\n2. Creating device with custom recovery config...");
    let mut custom_config = RecoveryConfig::default();
    custom_config.max_retry_attempts = 10;
    custom_config.health_check_interval_secs = 60;
    custom_config.enable_system_reboot = false;
    
    let custom_device = QwiicRelayDevice::new_with_config(0x26, custom_config.clone());
    println!("   Custom device created with ID: 0x{:02X}", custom_device.id);
    println!("   Max retry attempts: {}", custom_config.max_retry_attempts);
    println!("   Health check interval: {} seconds", custom_config.health_check_interval_secs);
    println!("   System reboot enabled: {}", custom_config.enable_system_reboot);
    
    // Test 3: Check health status
    println!("\n3. Checking health status...");
    let health = device.get_health_status();
    println!("   Is healthy: {}", health.is_healthy);
    println!("   Consecutive failures: {}", health.consecutive_failures);
    println!("   Firmware version: {:?}", health.firmware_version);
    
    // Test 4: Test relay operations (will fail if no hardware connected)
    println!("\n4. Testing relay operations (may fail without hardware)...");
    match device.set_relay(1, true) {
        Ok(_) => println!("   Successfully turned on relay 1"),
        Err(e) => println!("   Expected error (no hardware): {}", e),
    }
    
    match device.get_relay_state(1) {
        Ok(state) => println!("   Relay 1 state: {}", if state { "ON" } else { "OFF" }),
        Err(e) => println!("   Expected error (no hardware): {}", e),
    }
    
    println!("\n5. Testing all relays off...");
    device.all_off();
    println!("   All relays off command sent");
    
    println!("\n✓ All tests completed successfully!");
    println!("Note: Hardware-specific operations will fail without actual Qwiic relay connected.");
}