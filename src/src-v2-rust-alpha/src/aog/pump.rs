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
                let ovf_sensor_pin = sensor_pin.unwrap().into_input();

                // pump off
                pump_pin_out.set_high();

                // need more water?
                // oscillating_state_safety protects against faulty connections to float sensor
                let mut oscillating_state_safety = 0;
                while ovf_sensor_pin.is_high(){
                    if oscillating_state_safety > 500 && ovf_sensor_pin.is_low(){
                        // pump on
                        pump_pin_out.set_low();
                    } else {
                        // pump off
                        pump_pin_out.set_high();
                    }
                    oscillating_state_safety += 1;
                } 

                // pump off
                pump_pin_out.set_high();

                // this should make the pump pin available
                drop(pump_pin_out);

                halt_physical_pump(pump_thread.clone());
        
                // sleep for a random amount of time
                // let mut rng = rand::thread_rng();
                // let n1: u8 = rng.gen();
                // let n2:u64 = n1.into();
                // let n3:u64 = n2 * 100;
                // sleep(Duration::from_millis(n3));
                //sleep(Duration::from_millis(2000))

            };


          
            // if pin.is_ok(){
                // let mut pin_out = pin.unwrap().into_output();
                // if crate::aog::sensors::get_arduino_raw().contains(&pump_thread.sensor_flag){
                //     log::info!("Pump on");
                //     pin_out.set_low();
                // } else {
                //     log::info!("Pump off");
                //     pin_out.set_high();
                //     thread::sleep(Duration::from_millis(20000));
                // }
              
           


            // } else {
            //     halt_pump(pump_thread.clone());
            // }
        } else {
            halt_pump(pump_thread.clone());
        }
        
        match rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
                halt_pump(pump_thread.clone());
                break;
            }
            Err(TryRecvError::Empty) => {}
        }

        thread::sleep(Duration::from_millis(100));
    });
}

pub fn halt_pump(pump_thread: PumpThread){
    log::warn!("Halting Pump Thread: {}", pump_thread.id);
    halt_physical_pump(pump_thread.clone());
    stop(pump_thread.clone());
}

pub fn halt_physical_pump(pump_thread: PumpThread){
    let gpio = Gpio::new();
    if gpio.is_ok() {
        let pin = gpio.unwrap().get(pump_thread.gpio_pin);
        if pin.is_ok(){
            let mut pin_out = pin.unwrap().into_output();
            pin_out.set_high();
            thread::sleep(Duration::from_millis(4000));
        }
    }
    crate::aog::command::run(format!("gpio off {}", pump_thread.gpio_pin));
}

pub fn stop(pump_thread: PumpThread){
    pump_thread.tx.send(("stop".to_string()));
}
