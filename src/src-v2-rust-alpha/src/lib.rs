pub mod aog;
pub mod error;

// Re-export commonly used types for convenience
pub use aog::qwiic::{QwiicRelayDevice, RecoveryConfig, RelayHealthStatus, RelayError};
pub use error::{AogError, AogResult};

use clap::Parser;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::time::{SystemTime, UNIX_EPOCH};
use std::error::Error;

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
    pub sensor_logs: Vec<SensorLog>,
    pub pump_config: Option<PumpConfig>,  // New pump configuration
    pub https_bind_address: Option<String>,  // HTTPS server bind address (default: 127.0.0.1)
    pub https_bind_port: Option<u16>,  // HTTPS server port (default: 8443)
    pub command_api_bind_address: Option<String>,  // Command API bind address (default: 127.0.0.1)
    pub command_api_bind_port: Option<u16>,  // Command API port (default: 9443)
}
impl Config {
    pub fn new() -> Config {
        let sensor_logs :Vec<SensorLog> = Vec::new();

        let random_id: String = thread_rng().sample_iter(&Alphanumeric).take(100).map(char::from).collect();

        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0));

        // Generate a secure password hash for new installations
        let password_hash = match aog::auth::get_initial_password() {
            Ok(pwd) => match aog::auth::hash_password(&pwd) {
                Ok(hash) => hash,
                Err(_) => {
                    // Fallback: generate a random hash if initial password generation fails
                    // This ensures the system still initializes but with an unusable password
                    format!("$argon2$invalid${}$", random_id)
                }
            },
            Err(_) => {
                // Fallback: generate a random hash if initial password generation fails
                format!("$argon2$invalid${}$", random_id)
            }
        };
        
        Config{
            id: random_id, 
            encrypted_password: password_hash, 
            version_installed: VERSION.unwrap_or("unknown").to_string(), 
            boot_time: since_the_epoch.as_secs(), 
            sensor_logs, 
            is_hvac_kit_installed: false, 
            is_sensor_kit_installed: false, 
            photo_cycle_start: 6, 
            photo_cycle_end: 24, 
            sensor_kit_config: None, 
            pump_config: None, 
            power_type: "".to_string(), 
            tank_one_to_two_pump_pin: 17, 
            uv_light_pin: 27, 
            air_circulation_pin: 22,
            https_bind_address: Some("127.0.0.1".to_string()),
            https_bind_port: Some(8443),
            command_api_bind_address: Some("127.0.0.1".to_string()),
            command_api_bind_port: Some(9443),
        }
    }
    pub fn save(&self) -> Result<(), Box<dyn Error>>{
        std::fs::File::create("/opt/aog/data.json")
            .map_err(|e| format!("Failed to create data.json: {}", e))?;
        let j = serde_json::to_string(&self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        std::fs::write("/opt/aog/data.json", &j)
            .map_err(|e| format!("Failed to write data.json: {}", e))?;

        if self.sensor_logs.len() > 0 {
            if let Err(e) = std::fs::File::create("/opt/aog/data.bak.json") {
                log::warn!("Failed to create backup file: {}", e);
            } else {
                if let Ok(j) = serde_json::to_string(&self) {
                    if let Err(e) = std::fs::write("/opt/aog/data.bak.json", j) {
                        log::warn!("Failed to write backup file: {}", e);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn load(retries: i64) -> Result<Config, Box<dyn Error>>{

        if !std::path::Path::new("/opt/aog/data.json").exists(){
            let new_c = Config::new();
            if let Err(e) = new_c.save() {
                log::warn!("Failed to save initial config: {}", e);
            }
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
                            if let Err(e) = new_c.save() {
                                log::warn!("Failed to save new config: {}", e);
                            }
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
pub struct PumpConfig {
    pub continuous_mode: bool,  // Enable continuous pump operation
    pub photo_cycle_enabled: bool,  // Enable photo cycle scheduling for pumps
    pub photo_cycle_start_hour: u8,  // Hour to start (0-23)
    pub photo_cycle_end_hour: u8,  // Hour to end (0-23)
    pub safety_gpio_pin: Option<u8>,  // Safety GPIO pin for external switches
    pub pump_runtime_limit_seconds: u64,  // Maximum runtime in seconds (safety)
    pub pump_cooldown_seconds: u64,  // Cooldown period between runs
}

impl Default for PumpConfig {
    fn default() -> Self {
        PumpConfig {
            continuous_mode: false,
            photo_cycle_enabled: false,
            photo_cycle_start_hour: 6,
            photo_cycle_end_hour: 24,
            safety_gpio_pin: None,
            pump_runtime_limit_seconds: 300,  // 5 minutes default
            pump_cooldown_seconds: 60,  // 1 minute cooldown
        }
    }
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
    pub fn save(&self) -> Result<(), Box<dyn Error>>{
        std::fs::File::create("/opt/aog/sessions.json")
            .map_err(|e| format!("Failed to create sessions.json: {}", e))?;
        let j = serde_json::to_string(&self)
            .map_err(|e| format!("Failed to serialize sessions: {}", e))?;
        std::fs::write("/opt/aog/sessions.json", &j)
            .map_err(|e| format!("Failed to write sessions.json: {}", e))?;

        if self.sessions.len() > 0 {
            if let Err(e) = std::fs::File::create("/opt/aog/sessions.bak.json") {
                log::warn!("Failed to create sessions backup file: {}", e);
            } else {
                if let Ok(j) = serde_json::to_string(&self) {
                    if let Err(e) = std::fs::write("/opt/aog/sessions.bak.json", j) {
                        log::warn!("Failed to write sessions backup file: {}", e);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn load(retries: i64) -> Result<Sessions, Box<dyn Error>>{

        if !std::path::Path::new("/opt/aog/sessions.json").exists(){
            let new_c = Sessions::new();
            if let Err(e) = new_c.save() {
                log::warn!("Failed to save initial sessions: {}", e);
            }
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
                            if let Err(e) = new_c.save() {
                                log::warn!("Failed to save new sessions: {}", e);
                            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn cleanup_test_files() {
        let test_files = [
            "/opt/aog/data.json",
            "/opt/aog/data.bak.json",
            "/opt/aog/sessions.json",
            "/opt/aog/sessions.bak.json",
        ];
        
        for file in test_files.iter() {
            if Path::new(file).exists() {
                let _ = fs::remove_file(file);
            }
        }
    }

    fn setup_test_dir() {
        if !Path::new("/opt/aog").exists() {
            let _ = fs::create_dir_all("/opt/aog");
        }
    }

    #[test]
    fn test_args_default_values() {
        let args = Args::parse_from(&["test"]);
        assert_eq!(args.max_threads, 6);
        assert_eq!(args.port, 8443);
        assert_eq!(args.encrypt, false);
        assert_eq!(args.key, "aog");
    }

    #[test]
    fn test_config_new() {
        let config = Config::new();
        assert!(!config.id.is_empty());
        assert_eq!(config.id.len(), 100);
        // Verify password is hashed, not plaintext
        assert!(config.encrypted_password.starts_with("$argon2") || config.encrypted_password.starts_with("$argon2$invalid$"));
        assert_ne!(config.encrypted_password, "aog");
        assert_eq!(config.photo_cycle_start, 6);
        assert_eq!(config.photo_cycle_end, 24);
        assert_eq!(config.tank_one_to_two_pump_pin, 17);
        assert_eq!(config.uv_light_pin, 27);
        assert_eq!(config.air_circulation_pin, 22);
        assert_eq!(config.is_hvac_kit_installed, false);
        assert_eq!(config.is_sensor_kit_installed, false);
        assert!(config.sensor_logs.is_empty());
        assert!(config.pump_config.is_none());
    }

    #[test]
    fn test_config_save_and_load() {
        setup_test_dir();
        cleanup_test_files();
        
        let config = Config::new();
        let original_id = config.id.clone();
        config.save().expect("Failed to save config");
        
        assert!(Path::new("/opt/aog/data.json").exists());
        
        let loaded_config = Config::load(0).expect("Failed to load config");
        assert_eq!(loaded_config.id, original_id);
        // Verify password remains hashed after loading
        assert!(loaded_config.encrypted_password.starts_with("$argon2") || loaded_config.encrypted_password.starts_with("$argon2$invalid$"));
        
        cleanup_test_files();
    }

    #[test]
    fn test_config_backup_creation() {
        setup_test_dir();
        cleanup_test_files();
        
        let mut config = Config::new();
        config.sensor_logs.push(SensorLog {
            id: "test_log".to_string(),
            timestamp: 123456,
            s1_co2: "400".to_string(),
            s2_co2: "450".to_string(),
            avg_co2: "425".to_string(),
            humidity: "60".to_string(),
            temperature: "25".to_string(),
            is_tank_one_overflowed: false,
            is_tank_two_overflowed: false,
        });
        
        config.save().expect("Failed to save config with logs");
        
        assert!(Path::new("/opt/aog/data.json").exists());
        assert!(Path::new("/opt/aog/data.bak.json").exists());
        
        cleanup_test_files();
    }

    #[test]
    fn test_sensor_log_struct() {
        let sensor_log = SensorLog {
            id: "test_sensor".to_string(),
            timestamp: 1234567890,
            s1_co2: "500".to_string(),
            s2_co2: "550".to_string(),
            avg_co2: "525".to_string(),
            humidity: "65".to_string(),
            temperature: "22".to_string(),
            is_tank_one_overflowed: true,
            is_tank_two_overflowed: false,
        };
        
        assert_eq!(sensor_log.id, "test_sensor");
        assert_eq!(sensor_log.timestamp, 1234567890);
        assert_eq!(sensor_log.s1_co2, "500");
        assert_eq!(sensor_log.is_tank_one_overflowed, true);
        assert_eq!(sensor_log.is_tank_two_overflowed, false);
    }

    #[test]
    fn test_sessions_new() {
        let sessions = Sessions::new();
        assert!(sessions.sessions.is_empty());
    }

    #[test]
    fn test_sessions_save_and_load() {
        setup_test_dir();
        cleanup_test_files();
        
        let mut sessions = Sessions::new();
        sessions.sessions.push(Session {
            id: "session1".to_string(),
            delta: 10,
        });
        
        sessions.save().expect("Failed to save sessions");
        assert!(Path::new("/opt/aog/sessions.json").exists());
        assert!(Path::new("/opt/aog/sessions.bak.json").exists());
        
        let loaded_sessions = Sessions::load(0).expect("Failed to load sessions");
        assert_eq!(loaded_sessions.sessions.len(), 1);
        assert_eq!(loaded_sessions.sessions[0].id, "session1");
        assert_eq!(loaded_sessions.sessions[0].delta, 10);
        
        cleanup_test_files();
    }

    #[test]
    fn test_sensor_kit_config() {
        let sensor_kit = SensorKitConfig {
            dht11_pin: 7,
            tank_one_overflow: 4,
            tank_two_overflow: 2,
            analog_co2_pin: "A0".to_string(),
            enable_dht11: true,
            enable_analog_co2: true,
            enable_ccs811: false,
        };
        
        assert_eq!(sensor_kit.dht11_pin, 7);
        assert_eq!(sensor_kit.tank_one_overflow, 4);
        assert_eq!(sensor_kit.tank_two_overflow, 2);
        assert_eq!(sensor_kit.analog_co2_pin, "A0");
        assert!(sensor_kit.enable_dht11);
        assert!(sensor_kit.enable_analog_co2);
        assert!(!sensor_kit.enable_ccs811);
    }

    #[test]
    fn test_config_with_sensor_kit() {
        let mut config = Config::new();
        config.sensor_kit_config = Some(SensorKitConfig {
            dht11_pin: 8,
            tank_one_overflow: 5,
            tank_two_overflow: 3,
            analog_co2_pin: "A1".to_string(),
            enable_dht11: false,
            enable_analog_co2: true,
            enable_ccs811: true,
        });
        
        assert!(config.sensor_kit_config.is_some());
        let sensor_kit = config.sensor_kit_config.expect("Sensor kit config should be present");
        assert_eq!(sensor_kit.dht11_pin, 8);
        assert_eq!(sensor_kit.analog_co2_pin, "A1");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::new();
        let json = serde_json::to_string(&config).expect("Failed to serialize config");
        let deserialized: Config = serde_json::from_str(&json).expect("Failed to deserialize config");
        
        assert_eq!(config.encrypted_password, deserialized.encrypted_password);
        assert_eq!(config.photo_cycle_start, deserialized.photo_cycle_start);
        assert_eq!(config.photo_cycle_end, deserialized.photo_cycle_end);
    }

    #[test]
    fn test_pump_config_default() {
        let pump_config = PumpConfig::default();
        assert_eq!(pump_config.continuous_mode, false);
        assert_eq!(pump_config.photo_cycle_enabled, false);
        assert_eq!(pump_config.photo_cycle_start_hour, 6);
        assert_eq!(pump_config.photo_cycle_end_hour, 24);
        assert_eq!(pump_config.safety_gpio_pin, None);
        assert_eq!(pump_config.pump_runtime_limit_seconds, 300);
        assert_eq!(pump_config.pump_cooldown_seconds, 60);
    }

    #[test]
    fn test_config_with_pump_config() {
        let mut config = Config::new();
        config.pump_config = Some(PumpConfig {
            continuous_mode: true,
            photo_cycle_enabled: true,
            photo_cycle_start_hour: 8,
            photo_cycle_end_hour: 20,
            safety_gpio_pin: Some(25),
            pump_runtime_limit_seconds: 600,
            pump_cooldown_seconds: 120,
        });
        
        assert!(config.pump_config.is_some());
        let pump_config = config.pump_config.expect("Pump config should be present");
        assert!(pump_config.continuous_mode);
        assert!(pump_config.photo_cycle_enabled);
        assert_eq!(pump_config.photo_cycle_start_hour, 8);
        assert_eq!(pump_config.photo_cycle_end_hour, 20);
        assert_eq!(pump_config.safety_gpio_pin, Some(25));
        assert_eq!(pump_config.pump_runtime_limit_seconds, 600);
        assert_eq!(pump_config.pump_cooldown_seconds, 120);
    }
}

