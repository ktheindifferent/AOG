# Pump Control Features Documentation

## Overview
This document describes the enhanced pump control features implemented for the A.O.G. system, providing more flexible and safe pump operation modes.

## New Features

### 1. Continuous Operation Mode
- **Purpose**: Allows pumps to run continuously, bypassing sensor-based control
- **Use Case**: Useful for maintenance, testing, or when sensor control is not needed
- **Safety**: Includes periodic safety checks even in continuous mode

### 2. Photo Cycle Scheduling
- **Purpose**: Synchronizes pump operation with light schedules
- **Configuration**: 
  - `photo_cycle_start_hour`: Hour to start operation (0-23)
  - `photo_cycle_end_hour`: Hour to end operation (0-23)
- **Use Case**: Optimize pump operation during photosynthesis periods
- **Behavior**: Pumps will only run during the specified time window

### 3. Safety GPIO Pin Monitoring
- **Purpose**: Monitors external safety switches or sensors
- **Configuration**: `safety_gpio_pin`: GPIO pin number to monitor
- **Behavior**: 
  - Pin HIGH = Safe to operate
  - Pin LOW = Block pump operation
- **Use Case**: Connect external overflow sensors, emergency stops, or safety interlocks

### 4. Runtime Limits and Cooldown
- **Runtime Limit**: Maximum continuous operation time (default: 300 seconds)
- **Cooldown Period**: Rest time between pump runs (default: 60 seconds)
- **Purpose**: Prevent pump burnout and system overflow

## Implementation Details

### Pump Thread Structure
```rust
pub struct PumpThread {
    pub id: String,
    pub gpio_pin: u8,
    pub sensor_flag: String,
    pub running: bool,
    pub tx: std::sync::mpsc::Sender<String>,
    pub continuous: bool,
    pub photo_cycle_enabled: bool,
    pub photo_cycle_start: u8,
    pub photo_cycle_end: u8,
    pub safety_gpio_pin: Option<u8>,
}
```

### Configuration Structure
```rust
pub struct PumpConfig {
    pub continuous_mode: bool,
    pub photo_cycle_enabled: bool,
    pub photo_cycle_start_hour: u8,
    pub photo_cycle_end_hour: u8,
    pub safety_gpio_pin: Option<u8>,
    pub pump_runtime_limit_seconds: u64,
    pub pump_cooldown_seconds: u64,
}
```

## Web Interface

### UI Controls
The web interface provides a comprehensive control panel accessible via the "Water Pumps" button:

1. **Operation Mode Toggle**
   - Switch between sensor-based and continuous operation

2. **Photo Cycle Settings**
   - Enable/disable photo cycle
   - Set start and end hours

3. **Safety Configuration**
   - Enable safety GPIO monitoring
   - Configure safety pin number

4. **Runtime Limits**
   - Adjust maximum runtime
   - Set cooldown period

### API Endpoints
- `GET /api/pump/config` - Retrieve current pump configuration
- `POST /api/pump/config` - Update pump configuration
- `GET /api/pump/status` - Get current pump status

## Safety Features

### Multi-Layer Protection
1. **Overflow Detection**: Continuous monitoring of tank overflow sensors
2. **Safety GPIO**: External safety switch monitoring
3. **Runtime Limits**: Prevents continuous operation beyond safe limits
4. **Emergency Shutdown**: Immediate pump stop on safety condition
5. **Cooldown Enforcement**: Ensures pumps rest between cycles

### Safety Check Priority
1. Overflow sensors (highest priority)
2. Safety GPIO pin
3. Photo cycle schedule
4. Runtime limits
5. Sensor-based control (normal operation)

## Usage Examples

### Continuous Mode for Testing
```javascript
{
    "continuous_mode": true,
    "photo_cycle_enabled": false,
    "safety_gpio_pin": 24,
    "pump_runtime_limit_seconds": 120
}
```

### Photo Cycle with Safety
```javascript
{
    "continuous_mode": false,
    "photo_cycle_enabled": true,
    "photo_cycle_start_hour": 6,
    "photo_cycle_end_hour": 22,
    "safety_gpio_pin": 24
}
```

### Overnight Operation
```javascript
{
    "photo_cycle_enabled": true,
    "photo_cycle_start_hour": 22,
    "photo_cycle_end_hour": 6
}
```

## Testing

Run the test suite to verify pump control features:
```bash
cargo test --lib pump
cargo test --lib test_pump_config
```

## Migration Notes

### Existing Systems
- Default configuration maintains backward compatibility
- Sensor-based control remains the default mode
- New features are opt-in via configuration

### Configuration File
The pump configuration is stored in `/opt/aog/data.json` under the `pump_config` field.

## Troubleshooting

### Pump Not Running
1. Check overflow sensors - ensure no overflow condition
2. Verify safety GPIO pin is HIGH (if configured)
3. Check photo cycle schedule if enabled
4. Review system logs for error messages

### Safety Pin Issues
- Ensure proper pull-up/pull-down resistors on safety GPIO
- Verify GPIO pin number is correct
- Check wiring connections

### Photo Cycle Not Working
- Verify system time is correct
- Check start/end hours are valid (0-23)
- For overnight cycles, ensure start > end

## Future Enhancements
- Multiple pump profiles
- Schedule templates
- Remote monitoring and alerts
- Pump health monitoring
- Flow rate sensing
- Water level automation