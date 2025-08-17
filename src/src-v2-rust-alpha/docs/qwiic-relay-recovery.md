# Qwiic Relay Recovery Mechanism Documentation

## Overview
The Qwiic relay recovery mechanism provides automatic detection and recovery from relay communication failures, firmware incompatibilities, and hardware issues. This ensures the A.O.G. system can maintain operation without manual intervention.

## Features

### 1. Firmware Version Checking
- **Version Range**: Supports firmware versions 1.0 to 2.0
- **Automatic Detection**: Checks firmware version on initialization
- **Incompatibility Handling**: Alerts and prevents operation with unsupported firmware

### 2. Automatic Retry with Exponential Backoff
- **Initial Delay**: 100ms
- **Maximum Delay**: 5000ms  
- **Backoff Strategy**: Doubles delay after each failure (100ms → 200ms → 400ms → 800ms → 1600ms → 3200ms → 5000ms)
- **Maximum Attempts**: 5 (configurable)

### 3. Health Check Monitoring
- **Check Interval**: Every 30 seconds (configurable)
- **Status Tracking**:
  - Health state (healthy/unhealthy)
  - Last successful operation timestamp
  - Consecutive failure count
  - Current firmware version
  - Last error details

### 4. System Reboot Recovery
- **Trigger Condition**: 10 consecutive failures (configurable)
- **Grace Period**: 5-second delay before reboot
- **Alert**: Sends notification before reboot
- **Enable/Disable**: Can be toggled via configuration

### 5. Alert System
- **Local Logging**: All errors logged to `/opt/aog/output.log`
- **Webhook Support**: Optional webhook URL for external notifications
- **Alert Types**:
  - Communication failures
  - Firmware incompatibility
  - Critical failures requiring reboot

## Configuration

### RecoveryConfig Structure
```rust
pub struct RecoveryConfig {
    pub enable_auto_recovery: bool,           // Enable/disable recovery features
    pub enable_system_reboot: bool,           // Allow system reboot on critical failure
    pub max_retry_attempts: u32,              // Maximum retry attempts (default: 5)
    pub health_check_interval_secs: u64,      // Health check interval (default: 30)
    pub max_consecutive_failures: u32,        // Failures before reboot (default: 10)
    pub alert_webhook_url: Option<String>,    // Optional webhook for alerts
}
```

### Default Configuration
- Auto Recovery: Enabled
- System Reboot: Enabled
- Max Retry Attempts: 5
- Health Check Interval: 30 seconds
- Max Consecutive Failures: 10
- Alert Webhook: None

## Usage Examples

### Basic Usage (with default recovery)
```rust
let qwiic_device = QwiicRelayDevice::new(0x25);
qwiic_device.test(); // Initializes with recovery mechanism
```

### Custom Configuration
```rust
let recovery_config = RecoveryConfig {
    enable_auto_recovery: true,
    enable_system_reboot: false,  // Disable automatic reboot
    max_retry_attempts: 10,        // More retry attempts
    health_check_interval_secs: 60, // Check every minute
    max_consecutive_failures: 20,   // More tolerance
    alert_webhook_url: Some("https://hooks.slack.com/...".to_string()),
};

let qwiic_device = QwiicRelayDevice::new_with_config(0x25, recovery_config);
```

### Checking Health Status
```rust
let health_status = qwiic_device.get_health_status();
if health_status.is_healthy {
    println!("Relay is healthy, firmware: {:?}", health_status.firmware_version);
} else {
    println!("Relay unhealthy: {:?}", health_status.last_error);
}
```

### Manual Relay Control with Recovery
```rust
// Set relay with automatic retry on failure
match qwiic_device.set_relay(1, true) {
    Ok(_) => println!("Relay 1 turned on"),
    Err(e) => println!("Failed after retries: {}", e),
}

// Get relay state with recovery
match qwiic_device.get_relay_state(1) {
    Ok(state) => println!("Relay 1 is: {}", if state { "ON" } else { "OFF" }),
    Err(e) => println!("Failed to get state: {}", e),
}
```

## Error Types

### RelayError Enum
- **CommunicationFailure**: I2C communication errors
- **FirmwareIncompatible**: Firmware version outside supported range
- **InitializationFailure**: Failed to initialize relay module
- **OperationFailure**: Failed to execute relay operation

## Recovery Flow

1. **Operation Requested** → Execute with retry mechanism
2. **Failure Detected** → Log error, increment failure count
3. **Retry with Backoff** → Wait with exponential delay
4. **Max Retries Reached** → Update health status, send alert
5. **Critical Threshold** → Check consecutive failures
6. **System Reboot** → If enabled and threshold exceeded

## Monitoring and Debugging

### Log Messages
```
INFO: Qwiic Relay Firmware Version: 1.2
DEBUG: Attempting relay test (attempt 1/5)
WARN: relay test failed on attempt 1: Communication failure
INFO: Retrying relay test after 100ms delay
ERROR: Critical failure in relay test: Communication failure
ERROR: ALERT: Critical relay failure: relay test - Communication failure
ERROR: Maximum consecutive failures (10) reached
ERROR: Triggering system reboot due to critical relay failure
```

### Health Check Logs
```
DEBUG: Health check successful, firmware version: 1.2
WARN: Health check failed: Communication error
ERROR: Health check: Maximum consecutive failures reached
```

## Testing

The module includes comprehensive unit tests:
- Error display formatting
- Health status initialization
- Recovery configuration defaults
- Device initialization
- Exponential backoff calculation

Run tests with:
```bash
cargo test --lib aog::qwiic::tests
```

## Migration from Legacy Code

The new implementation maintains backward compatibility through legacy methods:
- `test_legacy()`: Original test method without recovery
- `all_off_legacy()`: Original relay control without retry
- `set_relay_legacy()`: Direct relay control
- `get_relay_state_legacy()`: Direct state query

To use legacy behavior, disable auto-recovery:
```rust
let mut recovery_config = RecoveryConfig::default();
recovery_config.enable_auto_recovery = false;
let qwiic_device = QwiicRelayDevice::new_with_config(0x25, recovery_config);
```

## Best Practices

1. **Always Enable Health Monitoring**: Start health monitor after successful initialization
2. **Configure Webhook Alerts**: Set up external monitoring for production systems
3. **Adjust Thresholds**: Tune retry and failure thresholds based on hardware reliability
4. **Log Analysis**: Regularly review logs for patterns of failures
5. **Test Recovery**: Periodically test recovery mechanisms in controlled environment

## Troubleshooting

### Common Issues

1. **Frequent Reboots**
   - Increase `max_consecutive_failures`
   - Check I2C wiring and connections
   - Verify power supply stability

2. **Slow Recovery**
   - Decrease `health_check_interval_secs`
   - Reduce `max_retry_attempts` for faster failure detection

3. **No Alerts**
   - Verify webhook URL is correct
   - Check network connectivity
   - Review log files for webhook errors

4. **Firmware Incompatibility**
   - Update relay firmware
   - Adjust `MIN_FIRMWARE_VERSION` and `MAX_FIRMWARE_VERSION` constants

## Future Enhancements

- [ ] Metrics collection and reporting
- [ ] Automatic firmware update capability
- [ ] Multiple relay device support
- [ ] Configurable retry strategies
- [ ] Integration with system monitoring tools
- [ ] Historical failure analysis