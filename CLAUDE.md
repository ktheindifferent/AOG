# CLAUDE.md - A.O.G. Codebase Documentation

## Project Overview
A.O.G. (Algae Oxygen Reactor) is an environmental control system that uses blue-green algae to convert CO2 into oxygen, improving indoor air quality. The system reduces indoor CO2 levels from dangerous levels (1000-2000ppm+) to safe levels (400-600ppm).

## Repository Structure

```
/root/repo/
├── src/
│   ├── src-v1-python/           # Legacy Python implementation
│   │   └── algaecore-raspian.py # Original Python control system
│   └── src-v2-rust-alpha/       # Current Rust implementation
│       ├── src/
│       │   ├── main.rs          # Application entry point
│       │   ├── setup.rs         # System configuration & initialization
│       │   ├── aog.rs           # Core module definitions
│       │   └── aog/             # Hardware control modules
│       │       ├── command.rs   # CLI interface
│       │       ├── gpio.rs      # GPIO pin management
│       │       ├── http.rs      # Web server & API
│       │       ├── lcd.rs       # LCD display control
│       │       ├── pump.rs      # Pump control logic
│       │       ├── qwiic.rs     # I2C relay control
│       │       ├── sensors.rs   # Sensor interfaces
│       │       ├── video.rs     # Camera streaming
│       │       └── gpio/        # GPIO subsystem
│       ├── www/                 # Web interface
│       │   ├── index.html       # Dashboard
│       │   ├── css/             # Stylesheets (SB Admin 2)
│       │   └── js/              # JavaScript (charts, controls)
│       └── sensorkit/           # Arduino sensor modules
├── media/                       # Project images & videos
├── BUILD.md                     # Hardware build instructions
├── README.md                    # Project introduction
├── ROADMAP.md                   # Development roadmap
└── LICENSE.md                   # MIT License

```

## Technology Stack

### Core Application (Rust)
- **Language**: Rust (Edition 2021)
- **Version**: 0.2.0
- **Platform**: Raspberry Pi (Linux ARM)

### Key Dependencies
```toml
rppal = "0.12.0"           # Raspberry Pi GPIO control
serde_json = "1.0.96"      # JSON configuration handling
rouille = "3.6.2"          # HTTP server with SSL
sds011 = "0.2.1"           # Air quality sensor support
serialport = "4.2.1"       # Serial communication for sensors
qwiic-lcd-rs = "0.1.1"     # I2C LCD display
qwiic-relay-rs = "0.1.1"   # I2C relay control
rscam = "0.5.5"            # Camera support
signal-hook = "0.3.9"      # Graceful shutdown handling
simple_logger = "1.13.6"   # Logging system
```

### Web Interface
- **Framework**: SB Admin 2 (Bootstrap-based admin template)
- **Frontend**: HTML5, CSS3, JavaScript (vanilla)
- **Charts**: Chart.js for sensor data visualization
- **Communication**: REST API + WebSocket for real-time updates

### Hardware Interfaces
- **GPIO**: Direct pin control via rppal
- **I2C**: LCD display, Qwiic relay modules
- **Serial/UART**: CO2 sensors, air quality monitors
- **Camera**: Pi Camera or USB webcam support

## System Architecture

### Hardware Components
1. **Sensors**
   - CO2 sensor (Serial/UART)
   - Temperature sensor
   - Humidity sensor
   - Air quality sensor (SDS011)

2. **Actuators**
   - Water pumps (GPIO controlled)
   - Air pump (Relay 1)
   - Drain pump (Relay 2)
   - Fill pump (Relay 3)
   - Auxiliary tank pump (Relay 4)
   - LED grow lights (Relay 1)

3. **Display & Interface**
   - I2C LCD display for local status
   - Web dashboard for remote monitoring
   - CLI for system management

### Software Modules

#### Core Services (`src/aog/`)
- **`command.rs`**: Command-line interface for system control
- **`http.rs`**: REST API server and WebSocket handler
- **`sensors.rs`**: Unified sensor data collection and validation
- **`video.rs`**: Camera streaming service

#### Hardware Control
- **`gpio.rs` + `gpio/`**: Thread-safe GPIO pin management
- **`pump.rs`**: Pump scheduling and control logic
- **`qwiic.rs`**: I2C relay module control
- **`lcd.rs`**: LCD display updates and formatting

#### System Management
- **`setup.rs`**: Installation verification and initial configuration
- **`main.rs`**: Application lifecycle and thread orchestration

## Configuration

### System Configuration File
Location: `/opt/aog/config.json`

Contains:
- Sensor thresholds and calibration
- Pump schedules and durations
- Light cycle timings
- Web server settings
- Hardware pin assignments

### Logging
- **Location**: `/opt/aog/output.log`
- **Format**: Timestamped entries with severity levels
- **Rotation**: Manual or on system restart

## Control Logic

### CO2 Management Algorithm
```
if CO2 > 1000ppm:
    - Increase light intensity (photosynthesis boost)
    - Activate air pump (circulation)
elif CO2 < 600ppm:
    - Reduce air circulation
    - Maintain minimal light levels
else:
    - Maintain current settings
```

### Pump Cycling
- **Fill Cycle**: Adds water/nutrients via Relay 3
- **Drain Cycle**: Removes waste water via Relay 2
- **Circulation**: Continuous movement via Relay 4
- **Safety**: Runtime limits prevent overflow

### Light Schedule
- **Day Mode** (18 hours): Full spectrum LED for algae growth
- **Night Mode** (6 hours): Reduced or no lighting
- **Automatic**: Based on configured schedule

## API Endpoints

### REST API (`http.rs`)
- `GET /api/status` - System status and sensor readings
- `GET /api/sensors` - Current sensor data
- `POST /api/pump/{id}` - Manual pump control
- `POST /api/relay/{id}` - Manual relay control
- `GET /api/config` - Current configuration
- `POST /api/config` - Update configuration

### WebSocket
- `/ws` - Real-time sensor data stream
- Updates every 5 seconds
- JSON formatted messages

## Development Workflow

### Building the Project
```bash
cd src/src-v2-rust-alpha
cargo build --release
```

### Running the System
```bash
sudo ./target/release/aog
```

### Installation Script
```bash
./setup.sh  # Automated setup for Raspberry Pi
```

## Testing
Currently no automated test suite exists. Manual testing required for:
- Hardware integration
- Sensor readings
- Pump control
- Web interface functionality

## Safety Features
1. **Graceful Shutdown**: SIGTERM handler for clean exit
2. **Relay Initialization**: Safe state on startup
3. **Sensor Validation**: Error detection and handling
4. **Pump Limits**: Runtime restrictions prevent overflow
5. **Temperature Monitoring**: System health checks
6. **Thread Safety**: Mutex-protected shared state

## Future Enhancements (from ROADMAP.md)
- ✅ Rust implementation (completed)
- USB webcam support (partial)
- Solenoid support for automatic tank management
- TensorFlow integration for algae growth detection
- Enhanced web interface
- Local Web API expansion

## License
MIT License - Open source for individual and commercial use

## Support
- GitHub: https://github.com/PixelCoda/AOG
- Issues: Report bugs and feature requests via GitHub Issues

## Notes for Development
- All hardware operations require sudo/root access
- GPIO pins are platform-specific (Raspberry Pi)
- Serial ports may vary (/dev/ttyUSB0, /dev/ttyAMA0)
- Web interface serves from compiled binary (www.zip embedded)
- Configuration changes require system restart