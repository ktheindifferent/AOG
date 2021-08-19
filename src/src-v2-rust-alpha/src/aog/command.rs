use crate::aog;


use rppal::gpio::Gpio;

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use sds011::{SDS011};
use std::thread::sleep;


// Gpio uses BCM pin numbering. BCM GPIO 23 is tied to physical pin 16.
// const GPIO_LED: u8 = 23;

pub fn run(command: String) -> Result<(), Box<dyn Error>>{


    let split = command.split(" ");
    let split_vec = split.collect::<Vec<&str>>();


    if command.starts_with("cls") || command.starts_with("clear"){
        aog::cls();
    }

    
    if command.starts_with("stats"){
        aog::print_stats();
    }

    if command.starts_with("tvoc"){
        println!("{}", aog::sensors::get_tvoc(aog::sensors::get_arduino_raw()));
    }

    if command.starts_with("temp"){
        println!("{}", aog::sensors::get_temperature(aog::sensors::get_arduino_raw()));
    }

    if command.starts_with("hum"){
        println!("{}", aog::sensors::get_humidity(aog::sensors::get_arduino_raw()));
    }

    if command.starts_with("co2"){
        println!("{}", aog::sensors::get_co2(aog::sensors::get_arduino_raw()));
    }

    if command.starts_with("pm25"){
        println!("{}", aog::sensors::get_pm25());
    }

    if command.starts_with("pm10"){
        println!("{}", aog::sensors::get_pm10());
    }

    if command.starts_with("arduino"){
        println!("{}", aog::sensors::get_arduino_raw());
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
        println!("gpio status:                  prints status of the gpio bus");
        println!("gpio [on/off] [gpio_bdm]:     change state of a gpio pin");
        println!("clear/cls:                    clears screen");
        println!("help [command]:               shows help");
    }


    if command == "test".to_string(){

       loop {
           let raw = aog::sensors::get_arduino_raw();

           if raw.contains("TOP_TANK_OVERFLOW: NONE"){
            run("gpio on 17".to_string());
           } else {
            run("gpio off 17".to_string());
           }

       }
    }


    Ok(())
}