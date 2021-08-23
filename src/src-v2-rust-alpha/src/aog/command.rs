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

pub fn run(cmd: String) -> Result<(), Box<dyn Error>>{

    let command = cmd.clone();

    let split = command.split(" ");
    let split_vec = split.collect::<Vec<&str>>();


    if command.starts_with("cls") || command.starts_with("clear"){
        aog::cls();
    }


    if command.starts_with("top_tank_pump_start") {
        thread::spawn(|| {
     
            loop {
    
                let gpio = Gpio::new();
    
                if gpio.is_ok() {
                    let pin = gpio.unwrap().get(17);
                    if pin.is_ok(){
                        let mut pin_out = pin.unwrap().into_output();
                        if aog::sensors::get_arduino_raw().contains("TOP_TANK_OVERFLOW: NONE"){
                            pin_out.set_low();
                        } else {
                            pin_out.set_high();
                            thread::sleep(Duration::from_millis(4000));
                        }
                    }
                }
            }
    
        });
    }
    
    if command.starts_with("install"){
        crate::setup::install();
    }

    if command.starts_with("uninstall"){
        crate::setup::uninstall();
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

    
    if command.clone() == "help".to_string(){
        println!("gpio status:                  prints status of the gpio bus");
        println!("gpio [on/off] [gpio_bdm]:     change state of a gpio pin");
        println!("clear/cls:                    clears screen");
        println!("help [command]:               shows help");
    }


    if command == "test".to_string(){
        let selected_pin = 17;
        let mut pin = Gpio::new().unwrap().get(selected_pin).unwrap().into_output();
        loop {
            let raw = aog::sensors::get_arduino_raw();

            if raw.contains("TOP_TANK_OVERFLOW: NONE"){
                pin.set_low();
            } else {
                pin.set_high();
            }

        }
    }

    // 0-21
    if command.starts_with("gpio"){
        if command == "gpio status".to_string(){
            aog::gpio_status::init();
        }

        if command.contains("on"){
            thread::spawn(move|| {
                let split = cmd.split(" ");
                let split_vec = split.collect::<Vec<&str>>();
                let selected_pin = split_vec[2].parse::<u8>().unwrap();
                let mut pin = Gpio::new().unwrap().get(selected_pin).unwrap().into_output();
                loop {
                    pin.set_low();
                    thread::sleep(Duration::from_millis(500));
                }
            });
        } else if command.contains("off"){
            thread::spawn(move|| {
                let split = cmd.split(" ");
                let split_vec = split.collect::<Vec<&str>>();
                let selected_pin = split_vec[2].parse::<u8>().unwrap();
                let mut pin = Gpio::new().unwrap().get(selected_pin).unwrap().into_output();
                loop {
                    pin.set_high();
                    thread::sleep(Duration::from_millis(500));
                    break;
                }
            });
        }
    }


    


    Ok(())
}