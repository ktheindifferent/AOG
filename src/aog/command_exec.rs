use crate::aog;
use std::error::Error;
use std::io;
use std::path::Path;

const HELP: &str = "Available commands:
  stats                         - Show current sensor readings
  temp | hum | co2 | tvoc        - Show a current air-kit reading
  pm25 | pm10 | ph               - Show particulate or calibrated pH readings
  t1_ovf | t2_ovf                - Show current overflow readings
  gpio status                   - Show GPIO header modes and levels
  relay status                  - Show relay states and assignments
  relay on <1-4> [seconds]       - Start an output; omit seconds for Start/Stop
  relay off <1-4|all>            - Stop outputs and pause schedules
  pump status                   - Show water-pump states and assignments
  pump fill | drain | cycle     - Start the relay assigned to that pump
  pump stop                     - Stop water pumps and pause schedules
  cls | clear                   - Clear the console screen
  help                          - Show this help";

fn sensor_value(name: &str) -> String {
    match name {
        "ph" => aog::ph_sensor::active_sensor()
            .get_status()
            .current_ph
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "N/A".into()),
        "t1_ovf" | "t2_ovf" => {
            let readings = aog::sensors::overflow_readings_in(Path::new("/opt/aog/sensors"));
            readings[usize::from(name == "t2_ovf")].as_str().into()
        }
        _ => aog::sensor_history::current_readings(Path::new("/opt/aog"))
            .into_iter()
            .find(|(key, _)| *key == name)
            .and_then(|(_, value)| value)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "N/A".into()),
    }
}

/// This exact parser is also the remote command allowlist. Never fall back to
/// console execution: credential changes are local-only, and output writes
/// must not bypass the relay controller's ownership/interlocks.
pub fn execute_with_output(cmd: String) -> Result<String, Box<dyn Error>> {
    let parts: Vec<_> = cmd.split_whitespace().collect();
    match parts.as_slice() {
        ["help"] => Ok(HELP.into()),
        ["cls" | "clear"] => {
            aog::cls();
            Ok("Screen cleared".into())
        }
        ["stats"] => {
            let mut output = String::from("Current sensor readings:\n");
            for (name, label, unit) in [
                ("temp", "Temperature", " °C"),
                ("hum", "Humidity", " %"),
                ("co2", "CO2", " ppm"),
                ("tvoc", "TVOC", " ppb"),
                ("pm25", "PM2.5", " µg/m³"),
                ("pm10", "PM10", " µg/m³"),
                ("ph", "pH", ""),
                ("t1_ovf", "Overflow sensor 1", ""),
                ("t2_ovf", "Overflow sensor 2", ""),
            ] {
                let value = sensor_value(name);
                let unit = if value == "N/A" { "" } else { unit };
                output.push_str(&format!("{label}: {value}{unit}\n"));
            }
            Ok(output.trim_end().into())
        }
        [name @ ("temp" | "hum" | "co2" | "tvoc" | "pm25" | "pm10" | "ph" | "t1_ovf" | "t2_ovf")] => {
            Ok(sensor_value(name))
        }
        ["gpio", "status"] => aog::gpio::status::snapshot(),
        ["relay", ..] | ["pump", ..] => aog::web_control::relay_command(&cmd)
            .map_err(|error| io::Error::other(error).into()),
        ["gpio", ..] => Err(io::Error::other(
            "Raw GPIO control is not supported by the Rust v2 relay controller. Use relay on/off with the assigned relay number."
        ).into()),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Unknown or malformed command; use help for supported commands").into()),
    }
}
