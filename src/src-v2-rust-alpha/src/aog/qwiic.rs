

use serde::{Serialize, Deserialize};
use qwiic_relay_rs::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QwiicRelayDevice {
    pub id: u16,
}
impl QwiicRelayDevice {
    pub fn new(id: u16) -> QwiicRelayDevice {
        QwiicRelayDevice { id }
    }
    pub fn test(&self){
        let qwiic_relay_config = crate::QwiicRelayConfig::default();
        let mut qwiic_relay_d = QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id);
        match qwiic_relay_d{
            Ok(mut qwiic_relay) => {
                let qwiic_relay_version = qwiic_relay.get_version();
                match qwiic_relay_version {
                    Ok(v) => {
                        log::info!("Qwiic Relay Firmware Version: {}", v);
            
                        qwiic_relay.set_all_relays_off().unwrap();
                        std::thread::sleep(std::time::Duration::from_secs(2));
            
                    },
                    Err(err) => {
                        log::error!("{}", err);
                        // TODO: Trigger a reboot if the Qwiic Relay firmware version is not supported.
                    }
                }        
            }, 
            Err(err) => {
                log::error!("{}", err);
                // TODO: Trigger a reboot if the Qwiic Relay can't be contacted
            }
        }    
    }

    pub fn all_off(&self){
        let qwiic_relay_config = crate::QwiicRelayConfig::default();
        let mut qwiic_relay_d = QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id);
        match qwiic_relay_d{
            Ok(mut qwiic_relay) => {
                qwiic_relay.set_all_relays_off().unwrap()
            }, 
            Err(err) => {
                log::error!("{}", err);
            }
        }    
    }
}
