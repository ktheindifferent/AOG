# A.O.G. System Overview

## Architecture Diagram
```
┌─────────────────────────────────────────────────────────────┐
│                     Web Interface (Browser)                  │
│                    (Dashboard & Controls)                    │
└──────────────────────┬──────────────────────────────────────┘
                       │ HTTP/WebSocket
┌──────────────────────▼──────────────────────────────────────┐
│                    AOG Core Application                      │
│                    (Rust - main.rs)                         │
│  ┌─────────────────────────────────────────────────────┐   │
│  │               Service Layer                          │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐           │   │
│  │  │  HTTP    │ │  Video   │ │ Command  │           │   │
│  │  │  Server  │ │  Stream  │ │   CLI    │           │   │
│  │  └──────────┘ └──────────┘ └──────────┘           │   │
│  └─────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │            Hardware Abstraction Layer               │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐          │   │
│  │  │  GPIO    │ │  Sensors │ │   LCD    │          │   │
│  │  │  Control │ │  Reader  │ │  Display │          │   │
│  │  └──────────┘ └──────────┘ └──────────┘          │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐          │   │
│  │  │  Pump    │ │  Relay   │ │  Qwiic   │          │   │
│  │  │  Control │ │  Control │ │   I2C    │          │   │
│  │  └──────────┘ └──────────┘ └──────────┘          │   │
│  └─────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
                       │ Hardware Interfaces
┌──────────────────────▼──────────────────────────────────────┐
│                    Physical Hardware                         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │
│  │   CO2    │ │   Temp   │ │ Humidity │ │   Air    │     │
│  │  Sensor  │ │  Sensor  │ │  Sensor  │ │ Quality  │     │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │
│  │  Water   │ │   Air    │ │  Lights  │ │   LCD    │     │
│  │  Pumps   │ │   Pump   │ │  (LED)   │ │ Display  │     │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘     │
│  ┌────────────────────────────────────────────────────┐   │
│  │          Algae Reactor Chamber                     │   │
│  │        (Blue-green algae culture)                  │   │
│  └────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

## System Flow

### 1. Initialization Phase
- Load configuration from `/opt/aog/config.json`
- Initialize hardware interfaces (GPIO, I2C, Serial)
- Start sensor monitoring threads
- Initialize LCD display
- Set up logging system
- Configure signal handlers for graceful shutdown

### 2. Main Operation Loop
```
┌─────────────┐
│   Start     │
└──────┬──────┘
       │
       ▼
┌─────────────────────┐
│  Read Sensors       │◄──────┐
│  (CO2, Temp, etc.)  │       │
└──────┬──────────────┘       │
       │                       │
       ▼                       │
┌─────────────────────┐       │
│  Analyze Conditions │       │
│  (CO2 levels, pH)   │       │
└──────┬──────────────┘       │
       │                       │
       ▼                       │
┌─────────────────────┐       │
│  Control Actuators  │       │
│  (Pumps, Lights)    │       │
└──────┬──────────────┘       │
       │                       │
       ▼                       │
┌─────────────────────┐       │
│  Update Display     │       │
│  (LCD, Web UI)      │       │
└──────┬──────────────┘       │
       │                       │
       ▼                       │
┌─────────────────────┐       │
│  Log Data          │        │
└──────┬──────────────┘       │
       │                       │
       ▼                       │
┌─────────────────────┐       │
│  Sleep/Wait         │───────┘
└─────────────────────┘
```

### 3. Control Logic

#### CO2 Management
- **High CO2 (>1000ppm)**: Increase light intensity, activate air pump
- **Normal CO2 (600-1000ppm)**: Maintain current settings
- **Low CO2 (<600ppm)**: Reduce air circulation

#### Water Management
- **Fill Cycle**: Relay 3 activates to add water/nutrients
- **Drain Cycle**: Relay 2 activates to remove waste water
- **Circulation**: Relay 4 maintains water movement

#### Light Control
- **Day Mode**: Full spectrum LED for photosynthesis
- **Night Mode**: Reduced or no lighting
- **Growth Mode**: Optimized light cycles for algae growth

## Key Modules

### Core Application (`main.rs`)
- Entry point and initialization
- Signal handling for graceful shutdown
- Thread management

### HTTP Server (`http.rs`)
- REST API endpoints
- WebSocket connections for real-time data
- Static file serving for web UI

### Sensor Module (`sensors.rs`)
- Continuous sensor polling
- Data validation and error handling
- Sensor calibration routines

### GPIO Control (`gpio/`)
- Hardware pin management
- Thread-safe status updates
- Interrupt handling

### Configuration (`setup.rs`)
- System configuration loading
- Environment setup
- Installation verification

## Data Flow
1. **Sensors** → Raw data collection
2. **Processing** → Data validation and conversion
3. **Storage** → Logging to `/opt/aog/output.log`
4. **Display** → LCD and Web UI updates
5. **Control** → Actuator commands based on thresholds

## Communication Protocols
- **I2C**: LCD display, Qwiic relay modules
- **Serial/UART**: CO2 sensors, air quality sensors
- **GPIO**: Direct pin control for pumps and lights
- **HTTP/REST**: Web interface communication
- **WebSocket**: Real-time sensor data streaming

## Safety Features
- Graceful shutdown on SIGTERM
- Relay initialization to safe states
- Sensor error detection and handling
- Pump runtime limits to prevent overflow
- Temperature monitoring for system health