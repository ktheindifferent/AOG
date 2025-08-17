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

// TODO - Add continuous bool flag
// TODO - Add photo_cycle bool flag and photo_cycle_start, photo_cycle_end
// TODO - Add safty_gpio_pin intger

use std::sync::mpsc::{self, TryRecvError};


use rppal::gpio::Gpio;

use std::sync::Mutex;


use std::sync::atomic::{AtomicBool};
use std::sync::Arc;
use std::thread;


use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};



#[derive(Debug, Clone)]
pub struct PumpThread {
    pub id: String,
    pub gpio_pin: u8,
    pub sensor_flag: String,
    pub running: bool,
    pub tx: std::sync::mpsc::Sender<String>,

}


impl Default for PumpThread {
    fn default () -> PumpThread {


        let random_id: String = thread_rng().sample_iter(&Alphanumeric).take(100).map(char::from).collect();

        let (tx, _rx) = mpsc::channel();

        PumpThread{id: random_id, gpio_pin: 17, sensor_flag: "T1_OVF: NONE".to_string(), running: false, tx}
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

        let gpio = Gpio::new();

        if gpio.is_ok() {
            let u_gpio = gpio.unwrap();
            let pump_pin = u_gpio.get(pump_thread_lock.gpio_pin);
            let sensor_pin = u_gpio.get(16);
                
            if sensor_pin.is_ok() && pump_pin.is_ok() {
                let mut pump_pin_out = pump_pin.unwrap().into_output();
                let ovf_sensor_pin = sensor_pin.unwrap().into_input();

               

                // pump off
                pump_pin_out.set_high();

                // need more water?
                // oscillating_state_safety protects against faulty connections to float sensor
                let mut oscillating_state_safety:u64 = 0;
                while ovf_sensor_pin.is_high(){
                    if oscillating_state_safety > 10 && ovf_sensor_pin.is_high(){
                        // pump on
                        log::debug!("Pump On");
                        pump_pin_out.set_low();
                    } else {
                        // pump off
                        log::debug!("Pump Off");
                        pump_pin_out.set_high();
                        oscillating_state_safety += 1;
                    }
                } 

                // pump off
                pump_pin_out.set_high();

                // this should make the pump pin available
                drop(pump_pin_out);

                // TODO - test speed
                // TODO - Make sure inbetween state doesn't disturb oscillating_state_safety
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
        };
        assert_eq!(pump.id, "test_pump");
        assert_eq!(pump.gpio_pin, 22);
        assert_eq!(pump.sensor_flag, "CUSTOM_FLAG");
        assert_eq!(pump.running, true);
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
            }));
            pumps.push(pump);
        }
        
        assert_eq!(pumps.len(), 5);
        
        for (i, pump) in pumps.iter().enumerate() {
            let pump_lock = pump.lock().unwrap();
            assert_eq!(pump_lock.id, format!("pump_{}", i));
            assert_eq!(pump_lock.gpio_pin, 17 + i as u8);
        }
    }
}
