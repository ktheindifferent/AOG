use crate::aog;


use rppal::gpio::Gpio;

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
// Gpio uses BCM pin numbering. BCM GPIO 23 is tied to physical pin 16.
// const GPIO_LED: u8 = 23;

pub fn run(command: String) -> Result<(), Box<dyn Error>>{


    let split = command.split(" ");
    let split_vec = split.collect::<Vec<&str>>();


    if command.starts_with("cls") || command.starts_with("clear"){
        aog::cls();
    }

 

    // 0-21
    if command.starts_with("gpio"){
        if command == "gpio status".to_string(){
            aog::gpio_status::init();
        }

        if command.contains("on"){
            let selected_pin = split_vec[2].parse::<u8>().unwrap();
            let mut pin = Gpio::new()?.get(selected_pin)?.into_output();
            loop {
                pin.set_low();
                thread::sleep(Duration::from_millis(500));
                break;
            }
        }

        if command.contains("off"){
            let selected_pin = split_vec[2].parse::<u8>().unwrap();
            let mut pin = Gpio::new()?.get(selected_pin)?.into_output();
            loop {
                pin.set_high();
                thread::sleep(Duration::from_millis(500));
                break;
            }
        }
    }


    if command == "help".to_string(){
        println!("clear/cls:                clears screen");
        println!("help [command]:           shows help");
    }


    Ok(())
}