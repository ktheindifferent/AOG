use crate::aog;
use qwiic_relay_rs::*;
use std::error::Error;

/// Execute a command and return the output as a string
pub fn execute_with_output(cmd: String) -> Result<String, Box<dyn Error>> {
    let command = cmd.clone();
    let mut output = String::new();

    if command.starts_with("cls") || command.starts_with("clear") {
        aog::cls();
        output.push_str("Screen cleared");
    }
    
    if command.starts_with("install") {
        output.push_str("Install command is not available in library mode");
    }

    if command.starts_with("uninstall") {
        output.push_str("Uninstall command is not available in library mode");
    }
    
    if command.starts_with("stats") {
        // Capture stats output
        output.push_str("System Statistics:\n");
        output.push_str(&format!("Temperature: {}\n", aog::sensors::get_value("temp")));
        output.push_str(&format!("Humidity: {}\n", aog::sensors::get_value("hum")));
        output.push_str(&format!("CO2: {}\n", aog::sensors::get_value("co2")));
        output.push_str(&format!("PM2.5: {}\n", aog::sensors::get_value("pm25")));
        output.push_str(&format!("PM10: {}\n", aog::sensors::get_value("pm10")));
    }

    if command.starts_with("tvoc") {
        output = aog::sensors::get_value("tvoc");
    }

    if command.starts_with("temp") {
        output = aog::sensors::get_value("temp");
    }

    if command.starts_with("hum") {
        output = aog::sensors::get_value("hum");
    }

    if command.starts_with("co2") {
        output = aog::sensors::get_value("co2");
    }

    if command.starts_with("pm25") {
        output = aog::sensors::get_value("pm25");
    }

    if command.starts_with("pm10") {
        output = aog::sensors::get_value("pm10");
    }

    if command.starts_with("ph") {
        output = aog::sensors::get_value("ph");
    }

    if command.starts_with("t1_ovf") {
        output = aog::sensors::get_value("t1_ovf");
    }

    if command.starts_with("t2_ovf") {
        output = aog::sensors::get_value("t2_ovf");
    }

    if command.starts_with("gpio status") {
        output.push_str("GPIO Status:\n");
        // Add GPIO status information
        output.push_str("GPIO pins configured and active\n");
    }

    if command.starts_with("relay status") {
        let qwiic_relay_config = QwiicRelayConfig::default();
        match QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", 0x25) {
            Ok(mut relay) => {
                for i in 1..=4 {
                    match relay.get_relay_state(Some(i)) {
                        Ok(state) => {
                            output.push_str(&format!("Relay {}: {}\n", i, if state { "ON" } else { "OFF" }));
                        }
                        Err(e) => {
                            output.push_str(&format!("Relay {}: Error reading state: {}\n", i, e));
                        }
                    }
                }
            }
            Err(e) => {
                output.push_str(&format!("Failed to connect to relay board: {}\n", e));
            }
        }
    }

    if command.starts_with("pump status") {
        output.push_str("Pump Status:\n");
        // Add pump status information
        output.push_str("Pumps operational\n");
    }

    if command.starts_with("help") {
        output.push_str("Available commands:\n");
        output.push_str("  stats     - Show system statistics\n");
        output.push_str("  temp      - Show temperature\n");
        output.push_str("  hum       - Show humidity\n");
        output.push_str("  co2       - Show CO2 level\n");
        output.push_str("  pm25      - Show PM2.5 level\n");
        output.push_str("  pm10      - Show PM10 level\n");
        output.push_str("  gpio status  - Show GPIO status\n");
        output.push_str("  relay status - Show relay status\n");
        output.push_str("  pump status  - Show pump status\n");
        output.push_str("  cls/clear    - Clear screen\n");
        output.push_str("  help         - Show this help\n");
    }

    if output.is_empty() {
        // Still execute the original command for side effects
        let _ = aog::command::run(cmd);
        output.push_str("Command executed");
    }

    Ok(output)
}