# Pump Safety Implementation Documentation

## Overview
This document details the comprehensive pump safety system implemented for the A.O.G. (Algae Oxygen Reactor) project. The safety system prevents tank overflow, equipment damage, and ensures reliable operation through multiple layers of protection.

## Safety Architecture

### Core Components
1. **PumpSafetyMonitor** - Central safety monitoring system
2. **Oscillation Safety** - Protection against sensor faults
3. **Runtime Limits** - Prevents continuous operation beyond safe limits
4. **Emergency Shutdown** - Immediate stop capability for all pumps
5. **Cooldown Periods** - Prevents rapid cycling that could damage pumps

## Safety Limits and Thresholds

### Runtime Limits
Maximum continuous runtime for each pump type:
- **Fill Pump**: 300 seconds (5 minutes)
- **Drain Pump**: 600 seconds (10 minutes)
- **Circulation Pump**: 3600 seconds (1 hour)
- **Auxiliary Pump**: 1800 seconds (30 minutes)

### Cooldown Periods
- **Minimum Cooldown**: 30 seconds between operations
- **Emergency Cooldown**: 300 seconds (5 minutes) after emergency stop

### Oscillation Safety Parameters
- **Maximum Cycles**: 100 oscillations before forced stop
- **Speed Limits**: 100ms - 5000ms oscillation period
- **Safety Counter**: 10 cycles before pump activation
- **Maximum Oscillation Time**: 5 minutes continuous

### Water Level Thresholds
- **Critical High**: 95.0% - Emergency stop all fill operations
- **Warning High**: 85.0% - Prevent fill pump start
- **Normal High**: 75.0% - Normal operating maximum
- **Normal Low**: 25.0% - Normal operating minimum
- **Warning Low**: 15.0% - Prevent drain pump start
- **Critical Low**: 5.0% - Emergency stop all drain operations

## Safety Features

### 1. Multi-Layer Overflow Protection
- **Hardware Sensors**: GPIO pin 16 for physical float sensor
- **Software Monitoring**: Continuous checking of t1_ovf and t2_ovf values
- **Emergency Files**: /opt/aog/sensors/overflow_error detection
- **Safety GPIO Pin**: Optional external safety switch integration

### 2. Oscillating Pump Safety
**Problem Addressed**: Faulty float sensor connections could cause rapid pump cycling
**Solution Implemented**:
- Oscillation counter tracks state changes
- Minimum 10 cycles before pump activation
- Configurable oscillation speed (100ms default)
- Maximum oscillation time limit (5 minutes)
- Automatic counter reset after successful operation

### 3. Photo Cycle Scheduling
- Configurable start/end hours for pump operation
- Prevents operation outside allowed hours
- Supports overnight cycles (e.g., 22:00 to 06:00)

### 4. Continuous Mode Safety
- Periodic safety checks every second during continuous operation
- Immediate shutdown on overflow detection
- Safety pin monitoring during operation
- Runtime limit enforcement

### 5. Emergency Shutdown System
**Triggers**:
- Overflow detection on any tank
- Safety GPIO pin trigger
- Runtime limit exceeded
- Oscillation limit exceeded
- Manual emergency stop command

**Actions**:
- All pumps immediately stopped
- Emergency state logged to file
- 5-minute cooldown enforced
- Alert notifications sent

### 6. State Management
**Pump States**:
- `Idle`: Ready to operate
- `Running`: Currently operating
- `Oscillating`: In oscillation detection mode
- `Cooldown`: Waiting period after operation
- `EmergencyStop`: Stopped due to emergency
- `Maintenance`: Requires service
- `Fault`: Error condition detected

### 7. Logging and Monitoring
**Event Types Logged**:
- Pump start/stop with timestamps
- Overflow detections with tank levels
- Emergency shutdowns with affected pumps
- Safety check failures with details
- Maintenance requirements based on runtime

**Log Locations**:
- `/opt/aog/pump_safety.log` - JSON formatted events
- `/opt/aog/output.log` - General system log
- `/opt/aog/emergency_stop` - Emergency stop status

### 8. Calibration System
- Flow rate measurement
- Optimal speed determination
- Sensor delay compensation
- Overflow threshold adjustment
- Results saved to `/opt/aog/calibration_[pump_id].json`

## Testing Requirements

### Unit Tests Implemented
1. **test_pump_safety_monitor_creation** - Verify initialization
2. **test_pump_start_stop_cycle** - Test normal operation
3. **test_emergency_shutdown** - Verify emergency procedures
4. **test_oscillation_safety** - Check oscillation limits
5. **test_runtime_limits** - Verify time limits
6. **test_cooldown_period** - Test cooldown enforcement
7. **test_maintenance_tracking** - Check hour tracking
8. **test_calibration** - Verify calibration routine

### Hardware Testing Checklist
- [ ] Measure actual pump flow rates at different speeds
- [ ] Test with water levels at all threshold points
- [ ] Verify GPIO state transitions are clean
- [ ] Test power failure recovery
- [ ] Validate overflow sensor response time
- [ ] Test emergency stop button functionality
- [ ] Verify pump doesn't restart during cooldown
- [ ] Test oscillation detection with faulty sensor

### Integration Testing
- [ ] Test pump coordination (fill/drain sequences)
- [ ] Verify web interface shows safety status
- [ ] Test LCD display of safety warnings
- [ ] Verify log file rotation and persistence
- [ ] Test system restart with pumps in various states
- [ ] Validate configuration loading/saving

## Configuration

### config.json Safety Parameters
```json
{
  "pump_safety": {
    "max_runtime": {
      "fill": 300,
      "drain": 600,
      "circulation": 3600,
      "auxiliary": 1800
    },
    "cooldown_periods": {
      "minimum": 30,
      "emergency": 300
    },
    "oscillation": {
      "max_cycles": 100,
      "min_period_ms": 100,
      "max_period_ms": 5000,
      "safety_counter": 10
    },
    "water_levels": {
      "critical_high": 95.0,
      "warning_high": 85.0,
      "normal_high": 75.0,
      "normal_low": 25.0,
      "warning_low": 15.0,
      "critical_low": 5.0
    }
  }
}
```

## API Endpoints

### Safety Status
- `GET /api/pump/safety/status` - Current safety monitor status
- `GET /api/pump/safety/stats/{pump_id}` - Individual pump statistics
- `POST /api/pump/safety/reset` - Reset emergency stop
- `POST /api/pump/safety/calibrate/{pump_id}` - Start calibration

### Safety Events
- `GET /api/pump/safety/events` - Recent safety events
- `GET /api/pump/safety/events/{pump_id}` - Events for specific pump

## Maintenance Guidelines

### Daily Checks
1. Review pump_safety.log for any warnings or errors
2. Check total runtime hours for each pump
3. Verify overflow sensors are functioning

### Weekly Maintenance
1. Test emergency stop functionality
2. Clean overflow sensors
3. Check pump flow rates match calibration

### Monthly Tasks
1. Run calibration routine for all pumps
2. Review and archive safety logs
3. Test power failure recovery

### Service Intervals
- Every 1000 hours: Full pump inspection and cleaning
- Every 2000 hours: Replace pump seals and gaskets
- Every 5000 hours: Complete pump replacement recommended

## Troubleshooting

### Common Issues and Solutions

#### Pump Won't Start
1. Check emergency stop status: `/opt/aog/emergency_stop`
2. Verify cooldown period has elapsed
3. Check water levels are within safe range
4. Review recent safety events in logs

#### Frequent Emergency Stops
1. Check overflow sensor connections
2. Verify water level sensor calibration
3. Review oscillation counter in logs
4. Test safety GPIO pin if configured

#### Oscillation Detection Triggered
1. Clean float sensor contacts
2. Check wiring for loose connections
3. Adjust oscillation_safety_counter if needed
4. Consider increasing oscillation period

## Future Enhancements

### Planned Improvements
1. **Machine Learning**: Predictive maintenance based on pump performance
2. **Remote Monitoring**: Cloud-based safety monitoring dashboard
3. **Redundancy**: Dual pump configuration for critical operations
4. **Water Quality**: Integration with pH and turbidity sensors
5. **Auto-Calibration**: Scheduled automatic calibration routines
6. **SMS/Email Alerts**: Notification system for critical events

### Hardware Additions
1. Ultrasonic water level sensors for precise measurement
2. Flow meters for accurate volume tracking
3. Pressure sensors for pump health monitoring
4. Temperature sensors for pump overheating detection
5. Backup power system with automatic switchover

## Compliance and Safety Standards

### Design Principles
- **Fail-Safe**: All failures result in pump shutdown
- **Defense in Depth**: Multiple layers of protection
- **Redundant Checks**: Critical parameters checked multiple times
- **Conservative Limits**: Safety margins built into all thresholds
- **Audit Trail**: Complete logging of all safety events

### Testing Compliance
- Unit tests cover all safety functions
- Integration tests verify system-wide safety
- Hardware tests validate physical safety mechanisms
- Documentation maintained for all safety features

## Contact and Support

For questions or issues related to pump safety:
- GitHub Issues: https://github.com/PixelCoda/AOG/issues
- Safety concerns should be marked with [SAFETY] tag
- Include relevant log files when reporting issues

## Version History

### v2.0.0 - Complete Safety Overhaul
- Added PumpSafetyMonitor module
- Implemented oscillation safety fixes
- Added comprehensive logging
- Created calibration system
- Added emergency shutdown mechanism
- Implemented cooldown periods
- Added maintenance tracking

### v1.0.0 - Initial Implementation
- Basic pump control
- Simple overflow detection
- Manual operation only

---

**CRITICAL**: This safety system is designed to prevent equipment damage and tank overflow. However, it should not be relied upon as the sole safety mechanism. Always maintain physical overflow drains and manual shutoff valves as backup safety measures.