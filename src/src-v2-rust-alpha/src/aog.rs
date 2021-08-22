pub mod command;
pub mod sensors;
pub mod gpio_status;
pub mod web;

use std::io::Error;
use std::io::{Write, stdin, stdout};
use std::path::{Path};

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use serde::{Serialize, Deserialize};
use shuteye::sleep;

use std::time::{SystemTime, UNIX_EPOCH};

use std::time::Duration;

use std::sync::Mutex;


use std::sync::Arc;

use savefile::prelude::*;

extern crate termion;

use termion::{color, style};

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");



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
    println!("");
    sensors_check_animation();
    println!("");

    let arduino_raw = sensors::get_arduino_raw();

    println!("           PM2.5:    {}", sensors::get_pm25());
    println!("            PM10:    {}", sensors::get_pm10());
    println!("             CO2:    {}", sensors::get_co2(arduino_raw.clone()));
    println!("            TVOC:    {}", sensors::get_tvoc(arduino_raw.clone())); 
    println!("        HUMIDITY:    {}", sensors::get_humidity(arduino_raw.clone())); 
    println!("     TEMPERATURE:    {}", sensors::get_temperature(arduino_raw.clone()));
    
    
    println!("");
}

pub fn print_logo(){
    println!("\n\n");
    println!(r"█████      ██████      ██████      ");
    println!(r"██   ██    ██    ██    ██          ");
    println!(r"███████    ██    ██    ██   ███    ");
    println!(r"██   ██    ██    ██    ██    ██    ");
    println!(r"██   ██ ██  ██████  ██  ██████  ██ ");                      
    println!(r"----------------------------------------------------------------------------");
    println!(r"v0.2.0-alpha");
    println!(r"----------------------------------------------------------------------------");
}

pub fn cls(){
    assert!( std::process::Command::new("cls").status().or_else(|_| std::process::Command::new("clear").status()).unwrap().success() );
    print_logo();
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
    pub encrypted_password: String,
    pub version_installed: String,
    pub boot_time: u64,
    pub enable_automatic_updates: bool,
    pub is_hvac_kit_installed: bool, 
    pub is_sensor_kit_installed: bool,
    pub photo_cycle_start: u8, //default 6
    pub photo_cycle_end: u8, //default 24
    pub power_type: String, // Grid, Solar, Etc.
    pub tank_one_to_two_pump_pin: usize, // default 17
    pub uv_light_pin: usize,  // default 27
    pub air_circulation_pin: usize,  // default 22
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

        let random_id: String = thread_rng().sample_iter(&Alphanumeric).take(100).map(char::from).collect();

        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        


        Config{id: random_id, encrypted_password: format!(""), version_installed: VERSION.unwrap_or("unknown").to_string(), boot_time: since_the_epoch.as_secs(), sensor_logs: sensor_logs, enable_automatic_updates: false, is_hvac_kit_installed: false, is_sensor_kit_installed: false, photo_cycle_start: 6, photo_cycle_end: 24, sensor_kit_config: None, power_type: "".to_string(), tank_one_to_two_pump_pin: 17, uv_light_pin: 27, air_circulation_pin: 22}
    }
}


#[derive(Serialize, Deserialize, Savefile, Debug, Clone)]
pub struct Sessions {
    pub sessions: Vec<Session>,
}

#[derive(Serialize, Deserialize, Savefile, Debug, Clone)]
pub struct Session {
    pub id: String,
    pub delta: u8
}


pub fn load_config() -> Result<Config, SavefileError> {

    if !Path::new("/opt/aog/config.bin").exists() {
        let mut server_data = Config::default();
        save_file("/opt/aog/config.bin", 0, &server_data).unwrap();
    }

    let result = load_file("/opt/aog/config.bin", 0);
    if result.is_ok(){
        Ok(result.unwrap())
    } else {
        let mut rng = rand::thread_rng();
        let n1: u8 = rng.gen();
        sleep(Duration::from_millis(n1.into()));
        load_config()
    }
}

pub fn load_sessions(config: Arc<Mutex<Config>>) -> Result<Sessions, SavefileError> {
    // Result<T, SavefileError>
    let result = load_file("/opt/aog/dat/sessions.bin", 0);
    if format!("{:?}", result).contains("Err("){
        let mut rng = rand::thread_rng();
        let n1: u8 = rng.gen();
        sleep(Duration::from_millis(n1.into()));
        println!("error loading session file -- trying again...{}", format!("{:?}", result));
        // Err(result.unwrap())
        load_sessions(Arc::clone(&config))
    } else {
        Ok(result.unwrap())
    }

}