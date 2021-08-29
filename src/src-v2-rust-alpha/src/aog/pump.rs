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

    // Ensure device has a valid GPIO bus before starting pump thread
    let gpio = Gpio::new();
    if gpio.is_err() {
        return;
    }

    log::info!("Starting pump thread: {}", pump_thread.id);

    thread::spawn(move || while !term_now.load(Ordering::Relaxed) {
        


        

        let gpio = Gpio::new();

        if gpio.is_ok() {
            let sensor_pin = gpio.unwrap().get(16);
                
            if sensor_pin.is_ok(){
                let ovf_sensor_pin = sensor_pin.unwrap().into_input_pullup();
                log::info!("ovf_sensor_pin: {}", ovf_sensor_pin.read());
           
            };


            //            let pin = gpio.unwrap().get(pump_thread.gpio_pin);
            // if pin.is_ok(){
            //     // let mut pin_out = pin.unwrap().into_output();
            //     // if crate::aog::sensors::get_arduino_raw().contains(&pump_thread.sensor_flag){
            //     //     log::info!("Pump on");
            //     //     pin_out.set_low();
            //     // } else {
            //     //     log::info!("Pump off");
            //     //     pin_out.set_high();
            //     //     thread::sleep(Duration::from_millis(20000));
            //     // }
              
           


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
    stop(pump_thread);
}

pub fn stop(pump_thread: PumpThread){
    pump_thread.tx.send(("stop".to_string()));
}
