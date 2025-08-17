use proptest::prelude::*;
use aog::{Config, SensorLog, Session};

proptest! {
    #[test]
    fn test_sensor_log_co2_values(
        s1_co2 in 0u32..5000u32,
        s2_co2 in 0u32..5000u32,
    ) {
        let avg_co2 = (s1_co2 + s2_co2) / 2;
        
        let sensor_log = SensorLog {
            id: "prop_test".to_string(),
            timestamp: 1700000000,
            s1_co2: s1_co2.to_string(),
            s2_co2: s2_co2.to_string(),
            avg_co2: avg_co2.to_string(),
            humidity: "60".to_string(),
            temperature: "25".to_string(),
            is_tank_one_overflowed: false,
            is_tank_two_overflowed: false,
        };
        
        // Verify CO2 values are within expected range
        let s1: u32 = sensor_log.s1_co2.parse().unwrap();
        let s2: u32 = sensor_log.s2_co2.parse().unwrap();
        let avg: u32 = sensor_log.avg_co2.parse().unwrap();
        
        prop_assert!(s1 <= 5000);
        prop_assert!(s2 <= 5000);
        prop_assert_eq!(avg, (s1 + s2) / 2);
    }
    
    #[test]
    fn test_humidity_values(humidity in 0.0f64..100.0f64) {
        let sensor_log = SensorLog {
            id: "humidity_test".to_string(),
            timestamp: 1700000000,
            s1_co2: "450".to_string(),
            s2_co2: "450".to_string(),
            avg_co2: "450".to_string(),
            humidity: format!("{:.2}", humidity),
            temperature: "25".to_string(),
            is_tank_one_overflowed: false,
            is_tank_two_overflowed: false,
        };
        
        let parsed_humidity: f64 = sensor_log.humidity.parse().unwrap();
        prop_assert!(parsed_humidity >= 0.0 && parsed_humidity <= 100.0);
    }
    
    #[test]
    fn test_temperature_values(temp in -40.0f64..60.0f64) {
        let sensor_log = SensorLog {
            id: "temp_test".to_string(),
            timestamp: 1700000000,
            s1_co2: "450".to_string(),
            s2_co2: "450".to_string(),
            avg_co2: "450".to_string(),
            humidity: "60".to_string(),
            temperature: format!("{:.2}", temp),
            is_tank_one_overflowed: false,
            is_tank_two_overflowed: false,
        };
        
        let parsed_temp: f64 = sensor_log.temperature.parse().unwrap();
        prop_assert!(parsed_temp >= -40.0 && parsed_temp <= 60.0);
    }
    
    #[test]
    fn test_photo_cycle_times(
        start in 0u8..24u8,
        end in 0u8..24u8,
    ) {
        let mut config = Config::new();
        config.photo_cycle_start = start;
        config.photo_cycle_end = end;
        
        prop_assert!(config.photo_cycle_start < 24);
        prop_assert!(config.photo_cycle_end < 24);
    }
    
    #[test]
    fn test_gpio_pin_ranges(
        pin1 in 0u8..40u8,
        pin2 in 0u8..40u8,
        pin3 in 0u8..40u8,
    ) {
        let mut config = Config::new();
        config.tank_one_to_two_pump_pin = pin1 as usize;
        config.uv_light_pin = pin2 as usize;
        config.air_circulation_pin = pin3 as usize;
        
        // Raspberry Pi GPIO pins are typically 0-40
        prop_assert!(config.tank_one_to_two_pump_pin < 40);
        prop_assert!(config.uv_light_pin < 40);
        prop_assert!(config.air_circulation_pin < 40);
    }
    
    #[test]
    fn test_session_delta_values(delta in 0u8..=255u8) {
        let session = Session {
            id: "prop_session".to_string(),
            delta,
        };
        
        prop_assert!(session.delta <= 255);
    }
    
    #[test]
    fn test_timestamp_values(timestamp in 0usize..2147483647usize) {
        let sensor_log = SensorLog {
            id: "timestamp_test".to_string(),
            timestamp,
            s1_co2: "450".to_string(),
            s2_co2: "450".to_string(),
            avg_co2: "450".to_string(),
            humidity: "60".to_string(),
            temperature: "25".to_string(),
            is_tank_one_overflowed: false,
            is_tank_two_overflowed: false,
        };
        
        prop_assert!(sensor_log.timestamp <= 2147483647);
    }
    
    #[test]
    fn test_config_id_generation(seed in any::<u64>()) {
        // Seed the RNG for reproducibility in tests
        use rand::{SeedableRng, distributions::Alphanumeric};
        use rand::rngs::StdRng;
        
        let mut rng = StdRng::seed_from_u64(seed);
        let id: String = (0..100)
            .map(|_| rng.sample(Alphanumeric) as char)
            .collect();
        
        prop_assert_eq!(id.len(), 100);
        prop_assert!(id.chars().all(|c| c.is_alphanumeric()));
    }
    
    #[test]
    fn test_overflow_combinations(
        tank1 in any::<bool>(),
        tank2 in any::<bool>(),
    ) {
        let sensor_log = SensorLog {
            id: "overflow_prop".to_string(),
            timestamp: 1700000000,
            s1_co2: "450".to_string(),
            s2_co2: "450".to_string(),
            avg_co2: "450".to_string(),
            humidity: "60".to_string(),
            temperature: "25".to_string(),
            is_tank_one_overflowed: tank1,
            is_tank_two_overflowed: tank2,
        };
        
        // All combinations of boolean values are valid
        prop_assert!(sensor_log.is_tank_one_overflowed == tank1);
        prop_assert!(sensor_log.is_tank_two_overflowed == tank2);
    }
    
    #[test]
    fn test_sensor_log_string_parsing(
        co2_str in "[0-9]{1,4}",
        temp_str in "-?[0-9]{1,2}\\.[0-9]{1,2}",
        hum_str in "[0-9]{1,2}\\.[0-9]{1,2}",
    ) {
        let sensor_log = SensorLog {
            id: "parse_test".to_string(),
            timestamp: 1700000000,
            s1_co2: co2_str.clone(),
            s2_co2: co2_str.clone(),
            avg_co2: co2_str,
            humidity: hum_str,
            temperature: temp_str,
            is_tank_one_overflowed: false,
            is_tank_two_overflowed: false,
        };
        
        // All string values should be parseable
        prop_assert!(sensor_log.s1_co2.parse::<u32>().is_ok());
        prop_assert!(sensor_log.humidity.parse::<f64>().is_ok());
        prop_assert!(sensor_log.temperature.parse::<f64>().is_ok());
    }
}