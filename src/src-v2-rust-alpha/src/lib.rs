use clap::Parser;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::time::{SystemTime, UNIX_EPOCH};
use std::error::Error;
use std::time::Duration;
use serde::{Serialize, Deserialize};

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

/// Simple program to greet a person
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value_t = 6)]
    pub max_threads: u8,
    #[arg(short, long, default_value_t = 8443)]
    pub port: u16,
    #[arg(short, long, default_value_t = false)]
    pub encrypt: bool,
    #[arg(short, long, default_value = "aog")]
    pub key: String,
}



#[derive(Serialize, Deserialize, Debug, Clone)]
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



#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub id: String,
    pub version_installed: String,
    pub boot_time: u64,
    pub encrypted_password: String,
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
impl Config {
    pub fn new() -> Config {
        let sensor_logs :Vec<SensorLog> = Vec::new();

        let random_id: String = thread_rng().sample_iter(&Alphanumeric).take(100).map(char::from).collect();

        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        Config{id: random_id, encrypted_password: format!("aog"), version_installed: VERSION.unwrap_or("unknown").to_string(), boot_time: since_the_epoch.as_secs(), sensor_logs, is_hvac_kit_installed: false, is_sensor_kit_installed: false, photo_cycle_start: 6, photo_cycle_end: 24, sensor_kit_config: None, power_type: "".to_string(), tank_one_to_two_pump_pin: 17, uv_light_pin: 27, air_circulation_pin: 22}
    }
    pub fn save(&self){
        std::fs::File::create("/opt/aog/data.json").expect("create failed");
        let j = serde_json::to_string(&self).unwrap();
        std::fs::write("/opt/aog/data.json", j).expect("Unable to write file");

        if self.sensor_logs.len() > 0 {
            std::fs::File::create("/opt/aog/data.bak.json").expect("create failed");
            let j = serde_json::to_string(&self).unwrap();
            std::fs::write("/opt/aog/data.bak.json", j).expect("Unable to write file");
        }
    }

    pub fn load(retries: i64) -> Result<Config, Box<dyn Error>>{

        if !std::path::Path::new("/opt/aog/data.json").exists(){
            let new_c = Config::new();
            new_c.save();
            return Ok(new_c);
        }

        let save_file = std::fs::read_to_string("/opt/aog/data.json");
        match save_file {
            Ok(save_data) => {
                let v: Result<Config, _> = serde_json::from_str(&save_data);
                match v {
                    Ok(v2) => {
                        return Ok(v2);
                    },
                    Err(e) => {
                        log::error!("{}", format!("Unable to parse save file: {}", e));
                        
                        if retries < 10 {
                            std::fs::copy("/opt/aog/data.bak.json", "/opt/aog/data.json")?;
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            return Self::load(retries + 1);
                        } else {
                            log::warn!("Unable to parse save file after 10 attempts....creating new save file.");
                            let new_c = Config::new();
                            new_c.save();
                            return Ok(new_c);
                        }
                 
                    }
                }
                
            },
            Err(e) => {
                log::error!("{}", format!("Unable to read save file: {}", e));
                if retries < 10 {
                    std::fs::copy("/opt/aog/data.bak.json", "/opt/aog/data.json")?;
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    return Self::load(retries + 1);
                } else {
                    log::warn!("Unable to read save file after 10 attempts....creating new save file.");
                    let new_c = Config::new();
                    new_c.save();
                    return Ok(new_c);
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SensorKitConfig {
    pub dht11_pin: u8, //default 7
    pub tank_one_overflow: u8, //default 4
    pub tank_two_overflow: u8, //default 2
    pub analog_co2_pin: String, //default A0
    pub enable_dht11: bool,
    pub enable_analog_co2: bool,
    pub enable_ccs811: bool, 
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Sessions {
    pub sessions: Vec<Session>,
}
impl Sessions {
    pub fn new() -> Sessions {
        let sessions :Vec<Session> = Vec::new();
        return Sessions{sessions};
    }
    pub fn save(&self){
        std::fs::File::create("/opt/aog/sessions.json").expect("create failed");
        let j = serde_json::to_string(&self).unwrap();
        std::fs::write("/opt/aog/sessions.json", j).expect("Unable to write file");

        if self.sessions.len() > 0 {
            std::fs::File::create("/opt/aog/sessions.bak.json").expect("create failed");
            let j = serde_json::to_string(&self).unwrap();
            std::fs::write("/opt/aog/sessions.bak.json", j).expect("Unable to write file");
        }
    }

    pub fn load(retries: i64) -> Result<Sessions, Box<dyn Error>>{

        if !std::path::Path::new("/opt/aog/sessions.json").exists(){
            let new_c = Sessions::new();
            new_c.save();
            return Ok(new_c);
        }

        let save_file = std::fs::read_to_string("/opt/aog/sessions.json");
        match save_file {
            Ok(save_data) => {
                let v: Result<Sessions, _> = serde_json::from_str(&save_data);
                match v {
                    Ok(v2) => {
                        return Ok(v2);
                    },
                    Err(e) => {
                        log::error!("{}", format!("Unable to parse save file: {}", e));
                        
                        if retries < 10 {
                            std::fs::copy("/opt/aog/sessions.bak.json", "/opt/aog/sessions.json")?;
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            return Self::load(retries + 1);
                        } else {
                            log::warn!("Unable to parse save file after 10 attempts....creating new save file.");
                            let new_c = Sessions::new();
                            new_c.save();
                            return Ok(new_c);
                        }
                 
                    }
                }
                
            },
            Err(e) => {
                log::error!("{}", format!("Unable to read save file: {}", e));
                if retries < 10 {
                    std::fs::copy("/opt/aog/sessions.bak.json", "/opt/aog/sessions.json")?;
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    return Self::load(retries + 1);
                } else {
                    log::warn!("Unable to read save file after 10 attempts....creating new save file.");
                    let new_c = Sessions::new();
                    new_c.save();
                    return Ok(new_c);
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Session {
    pub id: String,
    pub delta: u8
}

