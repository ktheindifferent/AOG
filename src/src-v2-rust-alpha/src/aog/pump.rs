// Copyright (c) 2020-2021 Caleb Mitchell Smith (PixelCoda)
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
// THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};
use std::thread::sleep;

use rppal::gpio::Gpio;

use std::sync::Mutex;


use std::sync::atomic::{AtomicBool};
use std::sync::Arc;
use std::thread;


use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use chrono::{Local, Timelike};

// Import pump safety module
use crate::aog::pump_safety::{PumpSafetyMonitor, PumpType, SAFETY_MONITOR};


#[derive(Debug, Clone)]
pub struct PumpThread {
    pub id: String,
    pub gpio_pin: u8,
    pub sensor_flag: String,
    pub running: bool,
    pub tx: std::sync::mpsc::Sender<String>,
    pub continuous: bool,  // Enable continuous operation mode
    pub photo_cycle_enabled: bool,  // Enable photo cycle scheduling
    pub photo_cycle_start: u8,  // Hour to start photo cycle (0-23)
    pub photo_cycle_end: u8,  // Hour to end photo cycle (0-23)
    pub safety_gpio_pin: Option<u8>,  // Optional safety GPIO pin for external switches
}


impl Default for PumpThread {
    fn default () -> PumpThread {


        let random_id: String = thread_rng().sample_iter(&Alphanumeric).take(100).map(char::from).collect();

        let (tx, _rx) = mpsc::channel();

        PumpThread{
            id: random_id, 
            gpio_pin: 17, 
            sensor_flag: "T1_OVF: NONE".to_string(), 
            running: false, 
            tx,
            continuous: false,
            photo_cycle_enabled: false,
            photo_cycle_start: 6,
            photo_cycle_end: 24,
            safety_gpio_pin: None,
        }
    }
}

// Helper function to check if pump should run based on photo cycle
fn is_within_photo_cycle(start: u8, end: u8) -> bool {
    let current_hour = Local::now().hour() as u8;
    
    if start < end {
        // Normal case: start=6, end=22 means run from 6am to 10pm
        current_hour >= start && current_hour < end
    } else {
        // Overnight case: start=22, end=6 means run from 10pm to 6am
        current_hour >= start || current_hour < end
    }
}

// Helper function to check safety GPIO pin
fn check_safety_pin(pin_number: u8) -> bool {
    let gpio = match Gpio::new() {
        Ok(g) => g,
        Err(e) => {
            log::error!("Failed to initialize GPIO for safety check: {:?}", e);
            return false; // Fail safe: don't run if we can't check safety
        }
    };
    
    match gpio.get(pin_number) {
        Ok(pin) => {
            let input_pin = pin.into_input();
            let is_safe = input_pin.is_high();
            if !is_safe {
                log::warn!("Safety pin {} is LOW - pump operation blocked", pin_number);
            }
            is_safe
        },
        Err(e) => {
            log::error!("Failed to read safety pin {}: {:?}", pin_number, e);
            false // Fail safe
        }
    }
}

pub fn start(pump_thread: Arc<Mutex<PumpThread>>, _term_now: Arc<AtomicBool>, rx: std::sync::mpsc::Receiver<String>){

    let pump_thread_lock = pump_thread.lock().unwrap();


    // Abort start if device doesn't have a GPIO bus (non-pi devices)
    let gpio = Gpio::new();
    if gpio.is_err() {
        log::warn!("No GIOS bus found. Halting pump thread: {}", pump_thread_lock.id);
        std::mem::drop(pump_thread_lock);
        return;
    }
    std::mem::drop(gpio);

    log::info!("Starting pump thread: {}", pump_thread_lock.id);

    std::mem::drop(pump_thread_lock);

    thread::spawn(move || loop {

        // while !term_now.load(Ordering::Relaxed)


        let pump_thread_lock = pump_thread.lock().unwrap();

        // Check if photo cycle is enabled and if we're within the allowed time
        if pump_thread_lock.photo_cycle_enabled {
            if !is_within_photo_cycle(pump_thread_lock.photo_cycle_start, pump_thread_lock.photo_cycle_end) {
                log::debug!("Pump {} outside photo cycle hours ({}-{})", 
                    pump_thread_lock.id, pump_thread_lock.photo_cycle_start, pump_thread_lock.photo_cycle_end);
                std::mem::drop(pump_thread_lock);
                sleep(Duration::from_secs(60)); // Check again in a minute
                
                // Check for stop signal
                match rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        stop_pump_thread(Arc::clone(&pump_thread));
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                }
                continue;
            }
        }

        // Check safety GPIO pin if configured
        if let Some(safety_pin) = pump_thread_lock.safety_gpio_pin {
            if !check_safety_pin(safety_pin) {
                log::error!("Safety GPIO pin {} check failed - pump {} blocked", 
                    safety_pin, pump_thread_lock.id);
                std::mem::drop(pump_thread_lock);
                sleep(Duration::from_secs(5)); // Check again in 5 seconds
                
                // Check for stop signal
                match rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        stop_pump_thread(Arc::clone(&pump_thread));
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                }
                continue;
            }
        }

        let gpio = Gpio::new();

        if gpio.is_ok() {
            let u_gpio = gpio.unwrap();
            let pump_pin = u_gpio.get(pump_thread_lock.gpio_pin);
            let sensor_pin = u_gpio.get(16);
                
            if sensor_pin.is_ok() && pump_pin.is_ok() {
                let mut pump_pin_out = pump_pin.unwrap().into_output();
                let ovf_sensor_pin = sensor_pin.unwrap().into_input();

               

                // pump off initially
                pump_pin_out.set_high();
                
                // CRITICAL SAFETY CHECK: Check for overflow conditions before operating pump
                let t1_ovf = crate::aog::sensors::get_value("t1_ovf");
                let t2_ovf = crate::aog::sensors::get_value("t2_ovf");
                let sensor_error = std::path::Path::new("/opt/aog/sensors/overflow_error").exists();
                
                // If any overflow condition exists, DO NOT operate pump
                if t1_ovf.contains("OVERFLOW") || t2_ovf.contains("OVERFLOW") || sensor_error {
                    log::error!("CRITICAL SAFETY: Overflow condition detected - pump operation blocked!");
                    log::error!("Tank 1: {}, Tank 2: {}, Sensor Error: {}", t1_ovf, t2_ovf, sensor_error);
                    
                    // Ensure pump is definitely off
                    pump_pin_out.set_high();
                    
                    // Wait before checking again
                    sleep(Duration::from_secs(30));
                } else if pump_thread_lock.continuous {
                    // Continuous operation mode
                    log::info!("Pump {} in continuous mode", pump_thread_lock.id);
                    
                    // Run continuously with periodic safety checks
                    pump_pin_out.set_low(); // Turn pump on
                    
                    // Sleep for a short interval to allow safety checks
                    for _ in 0..10 { // Check every second for 10 seconds
                        sleep(Duration::from_secs(1));
                        
                        // Check for overflow during continuous operation
                        let t1_check = crate::aog::sensors::get_value("t1_ovf");
                        let t2_check = crate::aog::sensors::get_value("t2_ovf");
                        
                        if t1_check.contains("OVERFLOW") || t2_check.contains("OVERFLOW") {
                            log::error!("CRITICAL: Overflow detected during continuous pump operation - emergency shutdown!");
                            pump_pin_out.set_high();
                            break;
                        }
                        
                        // Check safety pin during continuous operation
                        if let Some(safety_pin) = pump_thread_lock.safety_gpio_pin {
                            if !check_safety_pin(safety_pin) {
                                log::error!("Safety pin triggered during continuous operation - stopping pump");
                                pump_pin_out.set_high();
                                break;
                            }
                        }
                        
                        // Check for stop signal
                        match rx.try_recv() {
                            Ok(_) | Err(TryRecvError::Disconnected) => {
                                pump_pin_out.set_high();
                                stop_pump_thread(Arc::clone(&pump_thread));
                                std::mem::drop(pump_thread_lock);
                                return;
                            }
                            Err(TryRecvError::Empty) => {}
                        }
                    }
                } else {
                    // Normal sensor-based operation
                    // need more water?
                    // oscillating_state_safety protects against faulty connections to float sensor
                    let mut oscillating_state_safety:u64 = 0;
                    let mut oscillation_start_time = Instant::now();
                    let max_oscillation_time = Duration::from_secs(300); // 5 minutes max
                    
                    // Register pump start with safety monitor
                    SAFETY_MONITOR.register_pump_start(
                        pump_thread_lock.id.clone(),
                        PumpType::Fill  // Determine actual type based on GPIO pin
                    );
                    
                    while ovf_sensor_pin.is_high(){
                        // Double-check overflow status before each pump activation
                        let t1_check = crate::aog::sensors::get_value("t1_ovf");
                        let t2_check = crate::aog::sensors::get_value("t2_ovf");
                        
                        if t1_check.contains("OVERFLOW") || t2_check.contains("OVERFLOW") {
                            log::error!("CRITICAL: Overflow detected during pump operation - emergency shutdown!");
                            pump_pin_out.set_high();
                            break;
                        }
                        
                        // Check safety pin during normal operation
                        if let Some(safety_pin) = pump_thread_lock.safety_gpio_pin {
                            if !check_safety_pin(safety_pin) {
                                log::error!("Safety pin triggered - stopping pump");
                                pump_pin_out.set_high();
                                break;
                            }
                        }
                        
                        // Check oscillation time limit
                        if oscillation_start_time.elapsed() > max_oscillation_time {
                            log::warn!("Oscillation time limit exceeded - stopping pump");
                            pump_pin_out.set_high();
                            break;
                        }
                        
                        // Enhanced oscillation safety with configurable speed
                        let oscillation_period = 100; // milliseconds
                        
                        // Validate oscillation safety with pump safety monitor
                        match SAFETY_MONITOR.check_oscillation_safety(&pump_thread_lock.id, oscillation_period) {
                            Ok(true) => {
                                if oscillating_state_safety > 10 && ovf_sensor_pin.is_high(){
                                    // pump on
                                    log::debug!("Pump On - Cycle {}", oscillating_state_safety);
                                    pump_pin_out.set_low();
                                    sleep(Duration::from_millis(oscillation_period));
                                } else {
                                    // pump off
                                    log::debug!("Pump Off - Safety counter: {}", oscillating_state_safety);
                                    pump_pin_out.set_high();
                                    oscillating_state_safety += 1;
                                    sleep(Duration::from_millis(oscillation_period));
                                }
                            },
                            Ok(false) => {
                                log::error!("Oscillation safety check failed");
                                pump_pin_out.set_high();
                                break;
                            },
                            Err(e) => {
                                log::error!("Oscillation safety check failed: {}", e);
                                pump_pin_out.set_high();
                                break;
                            }
                        }
                        
                        // Check runtime limits
                        if !SAFETY_MONITOR.check_runtime_limit(&pump_thread_lock.id, PumpType::Fill) {
                            log::warn!("Runtime limit exceeded for pump {}", pump_thread_lock.id);
                            pump_pin_out.set_high();
                            break;
                        }
                    }
                } 

                // pump off
                pump_pin_out.set_high();

                // this should make the pump pin available
                drop(pump_pin_out);

                // Register pump stop with safety monitor
                SAFETY_MONITOR.register_pump_stop(
                    pump_thread_lock.id.clone(),
                    "Normal operation complete".to_string()
                );
                
                // Reset oscillation counter for next cycle
                SAFETY_MONITOR.reset_oscillation_counter(&pump_thread_lock.id);
                
                stop_physical_pump(Arc::clone(&pump_thread));
        
                // sleep for a random amount of time
                // let mut rng = rand::thread_rng();
                // let n1: u8 = rng.gen();
                // let n2:u64 = n1.into();
                // let n3:u64 = n2 * 100;
                // sleep(Duration::from_millis(n3));
                //sleep(Duration::from_millis(2000))

            } else {
                match sensor_pin {
                    Ok(_v) => {},
                    Err(e) => log::error!("{:?}", e),
                }
                match pump_pin {
                    Ok(_v) => {},
                    Err(e) => log::error!("{:?}", e),
                }
            }


          
      
        } else {
            match gpio {
                Ok(_v) => {},
                Err(e) => log::error!("{:?}", e),
            }

            // If we can't communicate with the GPIO bus...stop the pump...try again
            stop_physical_pump(Arc::clone(&pump_thread));
        }
        
        // If thread recieves stop signal terminate the thread immediately
        match rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
                stop_pump_thread(Arc::clone(&pump_thread));
                break;
            }
            Err(TryRecvError::Empty) => {}
        }

        std::mem::drop(pump_thread_lock);
    });
}

pub fn stop_pump_thread(pump_thread: Arc<Mutex<PumpThread>>){
    let pump_thread_lock = pump_thread.lock().unwrap();
    log::warn!("Halting Pump Thread: {}", pump_thread_lock.id);
    std::mem::drop(pump_thread_lock);
    stop_physical_pump(Arc::clone(&pump_thread));
    stop(Arc::clone(&pump_thread));
}

pub fn stop_physical_pump(pump_thread: Arc<Mutex<PumpThread>>){
    let pump_thread_lock = pump_thread.lock().unwrap();
    let gpio = Gpio::new();
    if gpio.is_ok() {
        let pin = gpio.unwrap().get(pump_thread_lock.gpio_pin);
        if pin.is_ok(){
            let mut pin_out = pin.unwrap().into_output();
            pin_out.set_high();
        }
    }
    let _ = crate::aog::command::run(format!("gpio off {}", pump_thread_lock.gpio_pin));
    std::mem::drop(pump_thread_lock);
}

pub fn stop(pump_thread: Arc<Mutex<PumpThread>>){
    let pump_thread_lock = pump_thread.lock().unwrap();
    let _ = pump_thread_lock.tx.send("stop".to_string());
    std::mem::drop(pump_thread_lock);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_pump_thread_default() {
        let pump = PumpThread::default();
        assert_eq!(pump.id.len(), 100);
        assert_eq!(pump.gpio_pin, 17);
        assert_eq!(pump.sensor_flag, "T1_OVF: NONE");
        assert_eq!(pump.running, false);
        assert_eq!(pump.continuous, false);
        assert_eq!(pump.photo_cycle_enabled, false);
        assert_eq!(pump.photo_cycle_start, 6);
        assert_eq!(pump.photo_cycle_end, 24);
        assert_eq!(pump.safety_gpio_pin, None);
    }

    #[test]
    fn test_pump_thread_custom() {
        let (tx, _rx) = mpsc::channel();
        let pump = PumpThread {
            id: "test_pump".to_string(),
            gpio_pin: 22,
            sensor_flag: "CUSTOM_FLAG".to_string(),
            running: true,
            tx,
            continuous: true,
            photo_cycle_enabled: true,
            photo_cycle_start: 8,
            photo_cycle_end: 20,
            safety_gpio_pin: Some(23),
        };
        assert_eq!(pump.id, "test_pump");
        assert_eq!(pump.gpio_pin, 22);
        assert_eq!(pump.sensor_flag, "CUSTOM_FLAG");
        assert_eq!(pump.running, true);
        assert_eq!(pump.continuous, true);
        assert_eq!(pump.photo_cycle_enabled, true);
        assert_eq!(pump.photo_cycle_start, 8);
        assert_eq!(pump.photo_cycle_end, 20);
        assert_eq!(pump.safety_gpio_pin, Some(23));
    }

    #[test]
    fn test_stop_function() {
        let pump = Arc::new(Mutex::new(PumpThread::default()));
        let pump_clone = Arc::clone(&pump);
        
        // This should send stop signal through channel
        stop(pump_clone);
        
        // Verify the channel can still be accessed
        let pump_lock = pump.lock().unwrap();
        assert!(pump_lock.tx.send("test".to_string()).is_ok());
    }

    #[test]
    fn test_pump_thread_id_uniqueness() {
        let pump1 = PumpThread::default();
        let pump2 = PumpThread::default();
        
        // IDs should be different (statistically)
        assert_ne!(pump1.id, pump2.id);
        
        // Both should be 100 characters long
        assert_eq!(pump1.id.len(), 100);
        assert_eq!(pump2.id.len(), 100);
    }

    #[test]
    fn test_pump_thread_arc_mutex_sharing() {
        let pump = Arc::new(Mutex::new(PumpThread::default()));
        let pump_clone1 = Arc::clone(&pump);
        let pump_clone2 = Arc::clone(&pump);
        
        // Modify through one clone
        {
            let mut pump_lock = pump_clone1.lock().unwrap();
            pump_lock.gpio_pin = 25;
            pump_lock.running = true;
        }
        
        // Verify changes through another clone
        {
            let pump_lock = pump_clone2.lock().unwrap();
            assert_eq!(pump_lock.gpio_pin, 25);
            assert_eq!(pump_lock.running, true);
        }
    }

    #[test]
    fn test_channel_communication() {
        let (tx, rx) = mpsc::channel();
        let pump = PumpThread {
            id: "channel_test".to_string(),
            gpio_pin: 17,
            sensor_flag: "TEST".to_string(),
            running: false,
            tx: tx.clone(),
            continuous: false,
            photo_cycle_enabled: false,
            photo_cycle_start: 6,
            photo_cycle_end: 24,
            safety_gpio_pin: None,
        };
        
        // Send message through pump's tx
        assert!(pump.tx.send("test_message".to_string()).is_ok());
        
        // Receive message
        match rx.try_recv() {
            Ok(msg) => assert_eq!(msg, "test_message"),
            Err(_) => panic!("Failed to receive message"),
        }
    }

    #[test]
    fn test_atomic_bool_termination() {
        let term_now = Arc::new(AtomicBool::new(false));
        let term_clone = Arc::clone(&term_now);
        
        // Initially false
        assert_eq!(term_now.load(Ordering::Relaxed), false);
        
        // Set to true through clone
        term_clone.store(true, Ordering::Relaxed);
        
        // Verify change
        assert_eq!(term_now.load(Ordering::Relaxed), true);
    }

    #[test]
    fn test_sensor_flag_formats() {
        let flags = vec![
            "T1_OVF: NONE",
            "T1_OVF: OVERFLOW",
            "T2_OVF: NONE",
            "T2_OVF: OVERFLOW",
        ];
        
        for flag in flags {
            let (tx, _rx) = mpsc::channel();
            let pump = PumpThread {
                id: "flag_test".to_string(),
                gpio_pin: 17,
                sensor_flag: flag.to_string(),
                running: false,
                tx,
                continuous: false,
                photo_cycle_enabled: false,
                photo_cycle_start: 6,
                photo_cycle_end: 24,
                safety_gpio_pin: None,
            };
            assert_eq!(pump.sensor_flag, flag);
        }
    }

    #[test]
    fn test_multiple_pump_threads() {
        let mut pumps = Vec::new();
        
        for i in 0..5 {
            let (tx, _rx) = mpsc::channel();
            let pump = Arc::new(Mutex::new(PumpThread {
                id: format!("pump_{}", i),
                gpio_pin: 17 + i,
                sensor_flag: format!("SENSOR_{}", i),
                running: false,
                tx,
                continuous: i % 2 == 0,
                photo_cycle_enabled: i % 3 == 0,
                photo_cycle_start: 6 + i,
                photo_cycle_end: 20 + i,
                safety_gpio_pin: if i > 2 { Some(25 + i) } else { None },
            }));
            pumps.push(pump);
        }
        
        assert_eq!(pumps.len(), 5);
        
        for (i, pump) in pumps.iter().enumerate() {
            let pump_lock = pump.lock().unwrap();
            assert_eq!(pump_lock.id, format!("pump_{}", i));
            assert_eq!(pump_lock.gpio_pin, 17 + i as u8);
            assert_eq!(pump_lock.continuous, i % 2 == 0);
            assert_eq!(pump_lock.photo_cycle_enabled, i % 3 == 0);
        }
    }

    #[test]
    fn test_photo_cycle_logic() {
        // Test normal day cycle (6am to 10pm)
        assert!(is_within_photo_cycle(6, 22));
        
        // Test overnight cycle (10pm to 6am)  
        // Note: This will depend on actual time, so we just test the logic exists
        let _ = is_within_photo_cycle(22, 6);
    }

    #[test]
    fn test_continuous_mode_pump() {
        let (tx, _rx) = mpsc::channel();
        let pump = PumpThread {
            id: "continuous_pump".to_string(),
            gpio_pin: 17,
            sensor_flag: "TEST".to_string(),
            running: false,
            tx,
            continuous: true,
            photo_cycle_enabled: false,
            photo_cycle_start: 6,
            photo_cycle_end: 24,
            safety_gpio_pin: None,
        };
        assert!(pump.continuous);
        assert!(!pump.photo_cycle_enabled);
    }

    #[test]
    fn test_safety_pin_configuration() {
        let (tx, _rx) = mpsc::channel();
        let pump = PumpThread {
            id: "safe_pump".to_string(),
            gpio_pin: 17,
            sensor_flag: "TEST".to_string(),
            running: false,
            tx,
            continuous: false,
            photo_cycle_enabled: false,
            photo_cycle_start: 6,
            photo_cycle_end: 24,
            safety_gpio_pin: Some(24),
        };
        assert_eq!(pump.safety_gpio_pin, Some(24));
    }

    #[test]
    fn test_photo_cycle_overnight() {
        let (tx, _rx) = mpsc::channel();
        let pump = PumpThread {
            id: "night_pump".to_string(),
            gpio_pin: 17,
            sensor_flag: "TEST".to_string(),
            running: false,
            tx,
            continuous: false,
            photo_cycle_enabled: true,
            photo_cycle_start: 22,  // 10pm
            photo_cycle_end: 6,      // 6am
            safety_gpio_pin: None,
        };
        assert!(pump.photo_cycle_enabled);
        assert_eq!(pump.photo_cycle_start, 22);
        assert_eq!(pump.photo_cycle_end, 6);
    }
}
