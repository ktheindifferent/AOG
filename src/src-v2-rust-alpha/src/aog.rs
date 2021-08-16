use std::io::Error;
use std::io::{Write, stdin, stdout};
use std::path::{Path};



use serde::{Serialize, Deserialize};
use shuteye::sleep;

use std::time::Duration;

use savefile::prelude::*;



#[derive(Serialize, Deserialize, Savefile, Debug, Clone)]
pub struct SensorLog {
    pub id: String,
    pub timestamp: usize,
    pub s1_co2: String,
    pub s2_co2: String,
    pub avg_co2: String,
    pub humidity: String,
    pub temperature: String,
    pub is_tank_one_overflowed: bool,
    pub is_tank_two_overflowed: bool
}

#[derive(Serialize, Deserialize, Savefile, Debug, Clone)]
pub struct Config {
    pub id: String,
    pub boot_time: usize,
    pub sensor_logs: Vec<SensorLog>,
    pub enable_automatic_updates: bool,
    pub is_hvac_kit_installed: bool, 
    pub is_sensor_kit_installed: bool,
    pub power_type: String, // Grid, Solar, Etc.
}

impl Default for Config {
    fn default () -> Config {

        let sensor_logs :Vec<SensorLog> = Vec::new();

        Config{id: "rand".to_string(), boot_time: 0, sensor_logs: sensor_logs, enable_automatic_updates: false, is_hvac_kit_installed: false, is_sensor_kit_installed: false, power_type: "".to_string()}
    }
}
