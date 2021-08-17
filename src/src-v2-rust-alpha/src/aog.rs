pub mod command;
pub mod sensors;
pub mod gpio_status;

use std::io::Error;
use std::io::{Write, stdin, stdout};
use std::path::{Path};



use serde::{Serialize, Deserialize};
use shuteye::sleep;

use std::time::Duration;

use savefile::prelude::*;

extern crate termion;

use termion::{color, style};

pub fn processing_animation(){
    let mut stdout = stdout();

    for i in 0..=100 {
        print!("\rProcessing {}%...", i);
        // or
        // stdout.write(format!("\rProcessing {}%...", i).as_bytes()).unwrap();

        stdout.flush().unwrap();
        sleep(Duration::from_millis(20));
    }
    println!();
}

pub fn sensors_check_animation(){
    let mut stdout = stdout();

    for i in 0..=99 {
        print!("\rChecking sensors {}%...", i);
        // or
        // stdout.write(format!("\rProcessing {}%...", i).as_bytes()).unwrap();

        stdout.flush().unwrap();
        sleep(Duration::from_millis(20));
    }
    println!();
}


pub fn print_stats(){
    sensors_check_animation();
    println!("{}{}PM2.5: {}{}{}   PM10: {}{}{}   CO2: {}{}{}   TEMP: {}{}{}   HUM: {}{}{}   TVOC: {}{}{} {}", color::Fg(color::Blue), style::Bold, color::Fg(color::White), sensors::get_pm25(), color::Fg(color::Blue), color::Fg(color::White), sensors::get_pm10(), color::Fg(color::Blue), color::Fg(color::White), sensors::get_co2(), color::Fg(color::Blue), color::Fg(color::White), sensors::get_temperature(), color::Fg(color::Blue), color::Fg(color::White), sensors::get_humidity(), color::Fg(color::Blue), color::Fg(color::White), sensors::get_tvoc(), color::Fg(color::Blue), style::Reset);
    println!(r"----------------------------------------------------------------------------");
}

pub fn print_logo(){
    println!("\n\n");
    println!(r"█████      ██████      ██████      ");
    println!(r"██   ██    ██    ██    ██          ");
    println!(r"███████    ██    ██    ██   ███    ");
    println!(r"██   ██    ██    ██    ██    ██    ");
    println!(r"██   ██ ██  ██████  ██  ██████  ██ ");                      
    println!(r"----------------------------------------------------------------------------");
    println!(r"v2.0.0-alpha");
    println!(r"----------------------------------------------------------------------------");
}

pub fn cls(){
    assert!( std::process::Command::new("cls").status().or_else(|_| std::process::Command::new("clear").status()).unwrap().success() );
    print_logo();
    print_stats();
}



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
    pub enable_automatic_updates: bool,
    pub is_hvac_kit_installed: bool, 
    pub is_sensor_kit_installed: bool,
    pub power_type: String, // Grid, Solar, Etc.
    pub sensor_kit_config: Option<SensorKitConfig>,
    pub sensor_logs: Vec<SensorLog>
}

#[derive(Serialize, Deserialize, Savefile, Debug, Clone)]
pub struct SensorKitConfig {
    pub dht11_pin: u8, //default 7
    pub tank_one_overflow: u8, //default 4
    pub tank_two_overflow: u8, //default 2
    pub analog_co2_pin: String, //default A0
    pub enable_dht11: bool,
    pub enable_analog_co2: bool,
    pub enable_ccs811: bool, 
}

impl Default for Config {
    fn default () -> Config {

        let sensor_logs :Vec<SensorLog> = Vec::new();

        Config{id: "rand".to_string(), boot_time: 0, sensor_logs: sensor_logs, enable_automatic_updates: false, is_hvac_kit_installed: false, is_sensor_kit_installed: false, sensor_kit_config: None, power_type: "".to_string()}
    }
}
