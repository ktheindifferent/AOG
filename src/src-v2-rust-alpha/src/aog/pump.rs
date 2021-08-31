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

use std::sync::mpsc::{self, Sender, Receiver, TryRecvError};
use std::io::{self, BufRead};

use rppal::gpio::Gpio;

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use std::thread::sleep;

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

        let (tx, rx) = mpsc::channel();

        PumpThread{id: random_id, gpio_pin: 17, sensor_flag: "T1_OVF: NONE".to_string(), running: false, tx}
    }
}

pub fn start(pump_thread: PumpThread, term_now: Arc<AtomicBool>, rx: std::sync::mpsc::Receiver<String>){

    // Abort start if device doesn't have a GPIO bus (non-pi devices)
    let gpio = Gpio::new();
    if gpio.is_err() {
        log::warn!("No GIOS bus found. Halting pump thread: {}", pump_thread.id);
        return;
    }

    log::info!("Starting pump thread: {}", pump_thread.id);

    thread::spawn(move || while !term_now.load(Ordering::Relaxed) {

        let gpio = Gpio::new();

        if gpio.is_ok() {
            let u_gpio = gpio.unwrap();
            let pump_pin = u_gpio.get(pump_thread.clone().gpio_pin);
            let sensor_pin = u_gpio.get(16);
                
            if sensor_pin.is_ok(){
                let mut pump_pin_out = pump_pin.unwrap().into_output();
                let ovf_sensor_pin = sensor_pin.unwrap().into_input_pullup();

               

                // pump off
                pump_pin_out.set_high();

                // need more water?
                // oscillating_state_safety protects against faulty connections to float sensor
                let mut oscillating_state_safety:u64 = 0;
                while ovf_sensor_pin.is_high(){
                    if oscillating_state_safety > 10 && ovf_sensor_pin.is_high(){
                        // pump on
                        // log::debug!("Pump On");
                        pump_pin_out.set_low();
                    } else {
                        // pump off
                        // log::debug!("Pump Off");
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
                stop_physical_pump(pump_thread.clone());
        
                // sleep for a random amount of time
                // let mut rng = rand::thread_rng();
                // let n1: u8 = rng.gen();
                // let n2:u64 = n1.into();
                // let n3:u64 = n2 * 100;
                // sleep(Duration::from_millis(n3));
                //sleep(Duration::from_millis(2000))

            };


          
      
        } else {
            // If we can't communicate with the GPIO bus...stop the pump...try again
            stop_physical_pump(pump_thread.clone());
        }
        
        // If thread recieves stop signal terminate the thread immediately
        match rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
                stop_pump_thread(pump_thread.clone());
                break;
            }
            Err(TryRecvError::Empty) => {}
        }
    });
}

pub fn stop_pump_thread(pump_thread: PumpThread){
    log::warn!("Halting Pump Thread: {}", pump_thread.id);
    stop_physical_pump(pump_thread.clone());
    stop(pump_thread.clone());
}

pub fn stop_physical_pump(pump_thread: PumpThread){
    let gpio = Gpio::new();
    if gpio.is_ok() {
        let pin = gpio.unwrap().get(pump_thread.gpio_pin);
        if pin.is_ok(){
            let mut pin_out = pin.unwrap().into_output();
            pin_out.set_high();
        }
    }
    crate::aog::command::run(format!("gpio off {}", pump_thread.gpio_pin));
}

pub fn stop(pump_thread: PumpThread){
    pump_thread.tx.send(("stop".to_string()));
}
