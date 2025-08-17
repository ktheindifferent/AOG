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


use rppal::gpio::Gpio;


use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use std::sync::Mutex;


use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use chrono::{Local, Timelike};


#[derive(Debug, Clone)]
pub struct GPIOThread {
    pub id: String,
    pub gpio_pin: u8,
    pub set_low_tx: std::sync::mpsc::Sender<String>,
    pub set_high_tx: std::sync::mpsc::Sender<String>,
    pub continuous: bool,  // Enable continuous operation mode
    pub photo_cycle_enabled: bool,  // Enable photo cycle scheduling
    pub photo_cycle_start: u8,  // Hour to start photo cycle (0-23)
    pub photo_cycle_end: u8,  // Hour to end photo cycle (0-23)
    pub safety_gpio_pin: Option<u8>,  // Optional safety GPIO pin for external switches
}


impl Default for GPIOThread {
    fn default () -> GPIOThread {


        let random_id: String = thread_rng().sample_iter(&Alphanumeric).take(100).map(char::from).collect();

        let (set_low_tx, _rx) = mpsc::channel();
        let (set_high_tx, _rx) = mpsc::channel();

        GPIOThread{
            id: random_id, 
            gpio_pin: 0, 
            set_low_tx, 
            set_high_tx,
            continuous: false,
            photo_cycle_enabled: false,
            photo_cycle_start: 6,
            photo_cycle_end: 24,
            safety_gpio_pin: None,
        }
    }
}

pub fn set_low(gpio_thread: Arc<Mutex<GPIOThread>>, term_now: Arc<AtomicBool>, rx: std::sync::mpsc::Receiver<String>){

    let _ = stop_high(Arc::clone(&gpio_thread));
    
    let gpio_thread_lock = match gpio_thread.lock() {
        Ok(lock) => lock,
        Err(e) => {
            log::error!("Failed to acquire GPIO thread lock: {:?}", e);
            return;
        }
    };

    // Abort start if device doesn't have a GPIO bus (non-pi devices)
    let gpio = Gpio::new();
    if gpio.is_err() {
        log::warn!("No GIOS bus found. Halting gpio thread: {}", gpio_thread_lock.id);
        std::mem::drop(gpio_thread_lock);
        return;
    }
    std::mem::drop(gpio);

    log::info!("Starting gpio-set-low thread: {}", gpio_thread_lock.id);
    
    let pin_num = gpio_thread_lock.gpio_pin;
    std::mem::drop(gpio_thread_lock);

    let gpio = Gpio::new();



    if let Ok(u_gpio) = gpio {
        if let Ok(gpio_pin) = u_gpio.get(pin_num) {
            let mut gpio_pin_out = gpio_pin.into_output();
            thread::spawn(move || while !term_now.load(Ordering::Relaxed) {

                gpio_pin_out.set_low();
                
                // If thread recieves stop signal terminate the thread immediately
                match rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        // stop_gpio_thread(gpio_thread.clone());
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                }
            });
        } else {
            log::error!("Failed to get GPIO pin {}", pin_num);
        }
 

    } else {
        match gpio {
            Ok(_v) => {},
            Err(e) => log::error!("{:?}", e),
        }
    }
}

pub fn set_high(gpio_thread: Arc<Mutex<GPIOThread>>, term_now: Arc<AtomicBool>, rx: std::sync::mpsc::Receiver<String>){

    let _ = stop_low(Arc::clone(&gpio_thread));

    let gpio_thread_lock = match gpio_thread.lock() {
        Ok(lock) => lock,
        Err(e) => {
            log::error!("Failed to acquire GPIO thread lock: {:?}", e);
            return;
        }
    };

    // Abort start if device doesn't have a GPIO bus (non-pi devices)
    let gpio = Gpio::new();
    if gpio.is_err() {
        log::warn!("No GIOS bus found. Halting gpio thread: {}", gpio_thread_lock.id);
        std::mem::drop(gpio_thread_lock);
        return;
    }

    log::info!("Starting gpio-set-high thread: {}", gpio_thread_lock.id);

    let pin_num = gpio_thread_lock.gpio_pin;
    std::mem::drop(gpio_thread_lock);

    let gpio = Gpio::new();



    if let Ok(u_gpio) = gpio {
        if let Ok(gpio_pin) = u_gpio.get(pin_num) {
            let mut gpio_pin_out = gpio_pin.into_output();
            thread::spawn(move || while !term_now.load(Ordering::Relaxed) {

                gpio_pin_out.set_high();
                
                // If thread recieves stop signal terminate the thread immediately
                match rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        // stop_gpio_thread(gpio_thread.clone());
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                }
            });
        } else {
            log::error!("Failed to get GPIO pin {}", pin_num);
        }
 

    } else {
        match gpio {
            Ok(_v) => {},
            Err(e) => log::error!("{:?}", e),
        }
    }
}


pub fn stop(gpio_thread: Arc<Mutex<GPIOThread>>){
    let _ = stop_low(Arc::clone(&gpio_thread));
    let _ = stop_high(Arc::clone(&gpio_thread));
}

pub fn stop_low(gpio_thread: Arc<Mutex<GPIOThread>>) -> Result<(), std::sync::mpsc::SendError<std::string::String>>{
    let gpio_thread_lock = match gpio_thread.lock() {
        Ok(lock) => lock,
        Err(e) => {
            log::error!("Failed to acquire GPIO thread lock: {:?}", e);
            return Err(std::sync::mpsc::SendError("lock failed".to_string()));
        }
    };
    let vv =  gpio_thread_lock.set_low_tx.send("stop".to_string());
    std::mem::drop(gpio_thread_lock);
    return vv;
}

pub fn stop_high(gpio_thread: Arc<Mutex<GPIOThread>>) -> Result<(), std::sync::mpsc::SendError<std::string::String>>{
    let gpio_thread_lock = match gpio_thread.lock() {
        Ok(lock) => lock,
        Err(e) => {
            log::error!("Failed to acquire GPIO thread lock: {:?}", e);
            return Err(std::sync::mpsc::SendError("lock failed".to_string()));
        }
    };
    let vv = gpio_thread_lock.set_high_tx.send("stop".to_string());
    std::mem::drop(gpio_thread_lock);
    return vv;
}
