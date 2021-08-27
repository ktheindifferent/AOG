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
    thread::spawn(move || loop {
        let gpio = Gpio::new();

        if gpio.is_ok() {
            let pin = gpio.unwrap().get(pump_thread.gpio_pin);
            if pin.is_ok(){
                let mut pin_out = pin.unwrap().into_output();
                if crate::aog::sensors::get_arduino_raw().contains(&pump_thread.sensor_flag){
                    pin_out.set_low();
                } else {
                    pin_out.set_high();
                    thread::sleep(Duration::from_millis(20000));
                }
            } else {
                halt_pump(pump_thread.clone());
            }
        } else {
            halt_pump(pump_thread.clone());
        }
        thread::sleep(Duration::from_millis(500));

        if term_now.load(Ordering::Relaxed){
            halt_pump(pump_thread.clone());
            break;
        }
        
        match rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
                
                halt_pump(pump_thread.clone());

                break;
            }
            Err(TryRecvError::Empty) => {}
        }
    });
}

pub fn halt_pump(pump_thread: PumpThread){
    println!("Halting Pump Thread: {}", pump_thread.id);
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
