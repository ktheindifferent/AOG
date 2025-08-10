# A.O.G. Development TODO List

## Recently Completed Tasks
- ✅ Created project_description.md with comprehensive project summary
- ✅ Created overview.md with system architecture and flow diagrams
- ✅ Analyzed project structure and identified missing test coverage
- ✅ Identified TODO comments and partially implemented features

## High Priority Tasks

### Testing Infrastructure
- [ ] Set up Rust testing framework
- [ ] Create unit tests for sensor module (`sensors.rs`)
- [ ] Create unit tests for pump control (`pump.rs`)
- [ ] Create unit tests for GPIO management (`gpio/`)
- [ ] Create integration tests for HTTP API endpoints
- [ ] Add mock hardware interfaces for testing without physical devices
- [ ] Set up continuous integration testing

### Partially Implemented Features (from TODO comments)

#### Main Application (`main.rs:145`)
- [ ] Implement background instance detection on localhost:9443
- [ ] Forward commands to localhost:8443 when background instance exists

#### Sensors Module (`sensors.rs`)
- [ ] Add pH sensor support (`sensors.rs:37`)
  - Reference: https://myhydropi.com/connecting-a-ph-sensor-to-a-raspberry-pi
- [ ] Implement water level sensor overflow protection (`sensors.rs:347, 358`)
  - Set DUAL_OVF_SENSOR to OVERFLOW state on broken pipe errors

#### Qwiic Relay Module (`qwiic.rs`)
- [ ] Add automatic reboot on unsupported firmware version (`qwiic.rs:42`)
- [ ] Add automatic reboot when Qwiic Relay can't be contacted (`qwiic.rs:48`)

#### Pump Control (`pump.rs`)
- [ ] Add continuous operation flag (`pump.rs:23`)
- [ ] Add photo cycle control with start/end times (`pump.rs:24`)
- [ ] Add safety GPIO pin configuration (`pump.rs:25`)
- [ ] Test pump speed control (`pump.rs:131`)
- [ ] Ensure oscillating_state_safety not disturbed (`pump.rs:132`)

#### GPIO Thread (`gpio/thread.rs`)
- [ ] Add continuous operation flag (`gpio/thread.rs:23`)
- [ ] Add photo cycle control with start/end times (`gpio/thread.rs:24`)
- [ ] Add safety GPIO pin configuration (`gpio/thread.rs:25`)

#### HTTP Server (`http.rs`)
- [ ] Add security flag for localhost-only connections (`http.rs:206`)

## Medium Priority Tasks

### Code Quality
- [ ] Add comprehensive error handling with custom error types
- [ ] Implement proper logging levels throughout codebase
- [ ] Add configuration validation on startup
- [ ] Create health check endpoint for monitoring

### Documentation
- [ ] Add inline documentation for all public APIs
- [ ] Create API documentation for HTTP endpoints
- [ ] Document hardware setup requirements
- [ ] Create troubleshooting guide

### Performance
- [ ] Optimize sensor polling intervals
- [ ] Implement caching for frequently accessed data
- [ ] Add database support for historical data storage
- [ ] Optimize web UI asset loading

## Low Priority Tasks

### Features
- [ ] Add data export functionality (CSV, JSON)
- [ ] Implement alerting system for critical conditions
- [ ] Add user authentication for web interface
- [ ] Create mobile app interface
- [ ] Add support for multiple reactor chambers

### Hardware Support
- [ ] Support additional sensor types
- [ ] Add support for different pump models
- [ ] Implement automatic calibration routines
- [ ] Add backup power monitoring

## Bug Fixes Needed
- [ ] Verify all relay states initialize correctly on startup
- [ ] Fix potential race conditions in threaded operations
- [ ] Handle serial port disconnections gracefully
- [ ] Improve error recovery in sensor reading loops

## Testing Checklist
- [ ] Test all relay combinations
- [ ] Test sensor failure scenarios
- [ ] Test pump runtime limits
- [ ] Test emergency shutdown procedures
- [ ] Test web UI on different browsers
- [ ] Test system under high CO2 conditions
- [ ] Test long-term stability (24+ hour runs)

## Next Steps
1. Set up testing framework
2. Create tests for existing functionality
3. Fix identified bugs from TODO comments
4. Implement missing safety features
5. Add comprehensive error handling
6. Document all changes

---
*Last Updated: 2025-08-10*
*Active Branch: terragon/maintain-docs-testing-features*