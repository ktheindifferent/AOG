# A.O.G. (Algae Oxygen Reactor) Project Description

## Project Summary
A.O.G. is an environmental control system that uses blue-green algae to convert CO2 into oxygen, improving indoor air quality. The system is designed to reduce indoor CO2 levels from dangerous levels (1000-2000ppm+) to safe levels (400-600ppm).

## Technology Stack
- **Backend**: Rust (v0.2.0, Edition 2021)
- **Hardware Interface**: Raspberry Pi GPIO control via rppal
- **Web Interface**: HTML/CSS/JavaScript with SB Admin 2 template
- **Sensors**: CO2, temperature, humidity, air quality monitoring
- **Actuators**: Pumps, relays, LCD display

## Core Components

### Hardware Control (`src/src-v2-rust-alpha/src/aog/`)
- **GPIO Management** (`gpio.rs`, `gpio/status.rs`, `gpio/thread.rs`): Controls hardware pins and status monitoring
- **Sensor Integration** (`sensors.rs`): Interfaces with environmental sensors (CO2, temperature, humidity)
- **Pump Control** (`pump.rs`): Manages water/nutrient pumps for algae system
- **LCD Display** (`lcd.rs`): Shows system status and sensor readings
- **Relay Control** (`qwiic.rs`): Controls power relays for lights, pumps, and auxiliary systems
  - Relay 1: Lights + Air
  - Relay 2: Drain
  - Relay 3: Fill
  - Relay 4: Aux Tank Pump

### Software Services
- **HTTP Server** (`http.rs`): Provides web API for monitoring and control
- **Video Stream** (`video.rs`): Camera feed for visual monitoring
- **Command Interface** (`command.rs`): CLI for system management
- **Configuration** (`setup.rs`): System initialization and configuration

### Web Interface (`src/src-v2-rust-alpha/www/`)
- Dashboard for real-time monitoring
- Control panels for system operations
- Charts and visualizations for sensor data
- Responsive design using Bootstrap framework

## Key Features
- Real-time CO2 monitoring and conversion
- Automated pump cycling for algae maintenance
- Environmental condition tracking
- Web-based remote monitoring and control
- Data logging and visualization
- Hardware safety interlocks

## Current Development Status
- Core functionality implemented
- Web interface operational
- Sensor integration complete
- No test suite currently exists
- Documentation being maintained

## License
MIT License - Open for individual and commercial use without restrictions

## Last Updated
2025-08-10