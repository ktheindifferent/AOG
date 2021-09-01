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


use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use std::sync::Mutex;


use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};



#[derive(Debug, Clone)]
pub struct GPIOThread {
    pub id: String,
    pub gpio_pin: u8,
    pub set_low_tx: std::sync::mpsc::Sender<String>,
    pub set_high_tx: std::sync::mpsc::Sender<String>,
}


impl Default for GPIOThread {
    fn default () -> GPIOThread {


        let random_id: String = thread_rng().sample_iter(&Alphanumeric).take(100).map(char::from).collect();

        let (set_low_tx, _rx) = mpsc::channel();
        let (set_high_tx, _rx) = mpsc::channel();

        GPIOThread{id: random_id, gpio_pin: 0, set_low_tx, set_high_tx}
    }
}

pub fn set_low(gpio_thread: Arc<Mutex<GPIOThread>>, term_now: Arc<AtomicBool>, rx: std::sync::mpsc::Receiver<String>){

    let _ = stop_high(Arc::clone(&gpio_thread));
    
    let mut gpio_thread_lock = gpio_thread.lock().unwrap();

    // Abort start if device doesn't have a GPIO bus (non-pi devices)
    let gpio = Gpio::new();
    if gpio.is_err() {
        log::warn!("No GIOS bus found. Halting gpio thread: {}", gpio_thread_lock.id);
        std::mem::drop(gpio_thread_lock);
        return;
    }

    log::info!("Starting gpio-set-low thread: {}", gpio_thread_lock.id);


    let gpio = Gpio::new();



    if gpio.is_ok() {
        let u_gpio = gpio.unwrap();
        let gpio_pin = u_gpio.get(gpio_thread_lock.gpio_pin);
        

        if gpio_pin.is_ok() {
            let mut gpio_pin_out = gpio_pin.unwrap().into_output();
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
        }
 

    }
}

pub fn set_high(gpio_thread: Arc<Mutex<GPIOThread>>, term_now: Arc<AtomicBool>, rx: std::sync::mpsc::Receiver<String>){

    let _ = stop_low(Arc::clone(&gpio_thread));

    let mut gpio_thread_lock = gpio_thread.lock().unwrap();

    // Abort start if device doesn't have a GPIO bus (non-pi devices)
    let gpio = Gpio::new();
    if gpio.is_err() {
        log::warn!("No GIOS bus found. Halting gpio thread: {}", gpio_thread_lock.id);
        std::mem::drop(gpio_thread_lock);
        return;
    }

    log::info!("Starting gpio-set-high thread: {}", gpio_thread_lock.id);


    let gpio = Gpio::new();



    if gpio.is_ok() {
        let u_gpio = gpio.unwrap();
        let gpio_pin = u_gpio.get(gpio_thread_lock.gpio_pin);
        

        if gpio_pin.is_ok() {
            let mut gpio_pin_out = gpio_pin.unwrap().into_output();
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
        }
 

    }
}


pub fn stop(gpio_thread: Arc<Mutex<GPIOThread>>){
    let _ = stop_low(Arc::clone(&gpio_thread));
    let _ = stop_high(Arc::clone(&gpio_thread));
}

pub fn stop_low(gpio_thread: Arc<Mutex<GPIOThread>>) -> Result<(), std::sync::mpsc::SendError<std::string::String>>{
    let mut gpio_thread_lock = gpio_thread.lock().unwrap();
    let vv =  gpio_thread_lock.set_low_tx.send("stop".to_string());
    std::mem::drop(gpio_thread_lock);
    return vv;
}

pub fn stop_high(gpio_thread: Arc<Mutex<GPIOThread>>) -> Result<(), std::sync::mpsc::SendError<std::string::String>>{
    let mut gpio_thread_lock = gpio_thread.lock().unwrap();
    let vv = gpio_thread_lock.set_high_tx.send("stop".to_string());
    std::mem::drop(gpio_thread_lock);
    return vv;
}
