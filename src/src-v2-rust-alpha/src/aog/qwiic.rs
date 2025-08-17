

use serde::{Serialize, Deserialize};
use qwiic_relay_rs::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QwiicRelayDevice {
    pub id: u16,
    pub aux_tank_pump_relay_id: Option<u16>,
    pub grow_light_relay_id: Option<u16>,
    pub water_pump_relay_id: Option<u16>,
    pub water_drain_relay_id: Option<u16>,
    pub air_circulation_relay_id: Option<u16>,

}
impl QwiicRelayDevice {
    pub fn new(id: u16) -> QwiicRelayDevice {
        QwiicRelayDevice { id,
            aux_tank_pump_relay_id: Some(4),
            grow_light_relay_id: Some(1),
            water_pump_relay_id: Some(3),
            water_drain_relay_id: Some(2),
            air_circulation_relay_id: Some(1)
        }
    }
    pub fn test(&self){
        let qwiic_relay_config = crate::QwiicRelayConfig::default();
        let qwiic_relay_d = QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id);
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
        let qwiic_relay_d = QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", self.id);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qwiic_relay_device_new() {
        let device = QwiicRelayDevice::new(0x18);
        
        assert_eq!(device.id, 0x18);
        assert_eq!(device.aux_tank_pump_relay_id, Some(4));
        assert_eq!(device.grow_light_relay_id, Some(1));
        assert_eq!(device.water_pump_relay_id, Some(3));
        assert_eq!(device.water_drain_relay_id, Some(2));
        assert_eq!(device.air_circulation_relay_id, Some(1));
    }

    #[test]
    fn test_qwiic_relay_device_custom() {
        let device = QwiicRelayDevice {
            id: 0x20,
            aux_tank_pump_relay_id: Some(1),
            grow_light_relay_id: Some(2),
            water_pump_relay_id: Some(3),
            water_drain_relay_id: Some(4),
            air_circulation_relay_id: None,
        };
        
        assert_eq!(device.id, 0x20);
        assert_eq!(device.aux_tank_pump_relay_id, Some(1));
        assert_eq!(device.grow_light_relay_id, Some(2));
        assert_eq!(device.water_pump_relay_id, Some(3));
        assert_eq!(device.water_drain_relay_id, Some(4));
        assert_eq!(device.air_circulation_relay_id, None);
    }

    #[test]
    fn test_qwiic_relay_device_clone() {
        let original = QwiicRelayDevice::new(0x18);
        let cloned = original.clone();
        
        assert_eq!(original.id, cloned.id);
        assert_eq!(original.aux_tank_pump_relay_id, cloned.aux_tank_pump_relay_id);
        assert_eq!(original.grow_light_relay_id, cloned.grow_light_relay_id);
        assert_eq!(original.water_pump_relay_id, cloned.water_pump_relay_id);
        assert_eq!(original.water_drain_relay_id, cloned.water_drain_relay_id);
        assert_eq!(original.air_circulation_relay_id, cloned.air_circulation_relay_id);
    }

    #[test]
    fn test_qwiic_relay_device_serialization() {
        let device = QwiicRelayDevice::new(0x18);
        
        // Serialize to JSON
        let json = serde_json::to_string(&device).unwrap();
        
        // Deserialize from JSON
        let deserialized: QwiicRelayDevice = serde_json::from_str(&json).unwrap();
        
        assert_eq!(device.id, deserialized.id);
        assert_eq!(device.aux_tank_pump_relay_id, deserialized.aux_tank_pump_relay_id);
        assert_eq!(device.grow_light_relay_id, deserialized.grow_light_relay_id);
    }

    #[test]
    fn test_relay_id_ranges() {
        // Test with valid relay IDs (1-4 typically for quad relay)
        let device = QwiicRelayDevice {
            id: 0x18,
            aux_tank_pump_relay_id: Some(1),
            grow_light_relay_id: Some(2),
            water_pump_relay_id: Some(3),
            water_drain_relay_id: Some(4),
            air_circulation_relay_id: Some(1),
        };
        
        assert!(device.aux_tank_pump_relay_id.unwrap() >= 1 && device.aux_tank_pump_relay_id.unwrap() <= 4);
        assert!(device.grow_light_relay_id.unwrap() >= 1 && device.grow_light_relay_id.unwrap() <= 4);
        assert!(device.water_pump_relay_id.unwrap() >= 1 && device.water_pump_relay_id.unwrap() <= 4);
        assert!(device.water_drain_relay_id.unwrap() >= 1 && device.water_drain_relay_id.unwrap() <= 4);
        assert!(device.air_circulation_relay_id.unwrap() >= 1 && device.air_circulation_relay_id.unwrap() <= 4);
    }

    #[test]
    fn test_i2c_addresses() {
        // Test common I2C addresses for Qwiic Relay
        let addresses = vec![0x18, 0x19, 0x1A, 0x1B];
        
        for addr in addresses {
            let device = QwiicRelayDevice::new(addr);
            assert_eq!(device.id, addr);
        }
    }

    #[test]
    fn test_optional_relay_ids() {
        let device = QwiicRelayDevice {
            id: 0x18,
            aux_tank_pump_relay_id: None,
            grow_light_relay_id: None,
            water_pump_relay_id: None,
            water_drain_relay_id: None,
            air_circulation_relay_id: None,
        };
        
        assert_eq!(device.aux_tank_pump_relay_id, None);
        assert_eq!(device.grow_light_relay_id, None);
        assert_eq!(device.water_pump_relay_id, None);
        assert_eq!(device.water_drain_relay_id, None);
        assert_eq!(device.air_circulation_relay_id, None);
    }
}
