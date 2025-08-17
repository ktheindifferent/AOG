# A.O.G. Testing Strategy

## Overview
This document outlines the comprehensive testing strategy for the A.O.G. (Algae Oxygen Reactor) codebase. Our testing approach ensures reliability, maintainability, and confidence in code changes.

## Test Coverage Goals
- **Target Coverage**: >80% for critical paths
- **Critical Modules**: sensors, pump control, GPIO operations, HTTP API
- **Safety-Critical Code**: 100% coverage for overflow detection and pump control

## Testing Pyramid

### 1. Unit Tests
Located in each module as `#[cfg(test)] mod tests`

**Coverage Areas:**
- **sensors.rs**: Arduino parsing, sensor reading, error handling
- **pump.rs**: Pump thread management, scheduling, control logic
- **qwiic.rs**: Relay device configuration and control
- **gpio modules**: Pin management and state control
- **lib.rs**: Configuration serialization, data structures

**Run Unit Tests:**
```bash
cd src/src-v2-rust-alpha
cargo test --lib
```

### 2. Integration Tests
Located in `tests/` directory

**Test Files:**
- `integration_test.rs`: Full system workflow tests
- `property_tests.rs`: Property-based testing with proptest

**Coverage Areas:**
- Configuration lifecycle
- Sensor data processing pipelines
- Multi-component interactions
- JSON serialization/deserialization
- File I/O operations

**Run Integration Tests:**
```bash
cd src/src-v2-rust-alpha
cargo test --test '*'
```

### 3. Property-Based Tests
Using `proptest` for generative testing

**Coverage Areas:**
- CO2 value ranges and calculations
- Temperature and humidity boundaries
- GPIO pin configurations
- Timestamp handling
- String parsing robustness

**Run Property Tests:**
```bash
cd src/src-v2-rust-alpha
cargo test property_tests
```

## Mocking Strategy

### Hardware Mocking
We use `mockall` for mocking hardware interfaces:
- GPIO operations
- Serial port communication
- I2C devices
- File system operations in `/opt/aog/`

### Example Mock Usage:
```rust
#[cfg(test)]
use mockall::automock;

#[automock]
trait SensorReader {
    fn read_co2(&self) -> Result<f64, Error>;
}
```

## CI/CD Pipeline

### GitHub Actions Workflow
`.github/workflows/rust-tests.yml`

**Pipeline Stages:**
1. **Format Check**: `cargo fmt`
2. **Linting**: `cargo clippy`
3. **Build**: `cargo build`
4. **Unit Tests**: `cargo test --lib`
5. **Integration Tests**: `cargo test --test '*'`
6. **Doc Tests**: `cargo test --doc`
7. **Coverage Report**: `cargo tarpaulin`
8. **Security Audit**: `cargo audit`

**Supported Rust Versions:**
- Stable (primary)
- Beta (compatibility check)
- Nightly (future compatibility)

## Running Tests Locally

### Prerequisites
```bash
# Create required directories
sudo mkdir -p /opt/aog/sensors
sudo chmod -R 777 /opt/aog

# Install development dependencies
cargo install cargo-tarpaulin
cargo install cargo-audit
```

### Full Test Suite
```bash
cd src/src-v2-rust-alpha

# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run specific test
cargo test test_sensor_log_valid_input

# Run tests in parallel
cargo test -- --test-threads=4
```

### Coverage Analysis
```bash
# Generate coverage report
cargo tarpaulin --out Html

# View coverage report
open tarpaulin-report.html
```

### Performance Testing
```bash
# Run benchmarks (if available)
cargo bench

# Run tests with optimization
cargo test --release
```

## Test Data Management

### Test Fixtures
- Located in `tests/fixtures/` (if needed)
- Use `tempfile` crate for temporary test files
- Clean up test artifacts automatically

### Environment Variables
Tests may set these for isolation:
- `AOG_TEST_DIR`: Override `/opt/aog/` path
- `AOG_SENSOR_DIR`: Override sensor data directory

## Writing New Tests

### Guidelines
1. **Test Naming**: Use descriptive names like `test_pump_overflow_protection`
2. **Isolation**: Each test should be independent
3. **Assertions**: Use clear, specific assertions
4. **Documentation**: Comment complex test logic
5. **Error Cases**: Test both success and failure paths

### Test Template
```rust
#[test]
fn test_feature_description() {
    // Arrange
    let input = setup_test_data();
    
    // Act
    let result = function_under_test(input);
    
    // Assert
    assert_eq!(result, expected_value);
}
```

## Continuous Improvement

### Metrics to Track
- Code coverage percentage
- Test execution time
- Test flakiness rate
- Bug escape rate

### Regular Reviews
- Monthly: Review test coverage gaps
- Quarterly: Update testing strategy
- Per Release: Full regression testing

## Troubleshooting

### Common Issues

**Permission Errors:**
```bash
# Fix: Ensure test directories exist and have proper permissions
sudo mkdir -p /opt/aog/sensors
sudo chmod -R 777 /opt/aog
```

**Serial Port Tests Failing:**
```bash
# Fix: Mock serial ports in tests or skip hardware tests
cargo test --lib  # Skip integration tests requiring hardware
```

**Flaky Tests:**
- Add retry logic for network-dependent tests
- Use `serial_test` crate for tests requiring exclusive access
- Increase timeouts for slow operations

## Test Categories

### Safety-Critical Tests
Must pass before deployment:
- Overflow detection
- Pump control limits
- Emergency shutdown
- Sensor validation

### Regression Tests
Prevent previously fixed bugs:
- Add test for each bug fix
- Document original issue in test comment
- Link to issue tracker

### Smoke Tests
Quick validation suite:
```bash
cargo test --test smoke_tests
```

## Hardware-in-the-Loop Testing

For testing with actual hardware:
1. Set up Raspberry Pi test environment
2. Connect mock sensors/actuators
3. Run hardware integration tests
4. Monitor GPIO states and serial output

## Security Testing

### Dependency Auditing
```bash
cargo audit
```

### Input Validation Tests
- Test boundary values
- Test invalid input handling
- Test injection attacks prevention

## Performance Benchmarks

### Baseline Metrics
- Sensor reading: <100ms
- HTTP API response: <200ms
- Configuration save: <50ms
- Pump control loop: <10ms

## Documentation

### Test Documentation
- Document test purpose in comments
- Link tests to requirements
- Maintain test coverage reports

### API Documentation Tests
```bash
cargo test --doc
```

## Future Enhancements

### Planned Improvements
- [ ] Mutation testing with `cargo-mutants`
- [ ] Fuzz testing for parser functions
- [ ] Contract testing for API endpoints
- [ ] Visual regression testing for web UI
- [ ] Load testing for concurrent operations
- [ ] Chaos engineering for resilience

## Support

For testing-related questions:
- Review this document
- Check CI logs for failures
- Open an issue with test label
- Include test output in bug reports