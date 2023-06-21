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

use crate::aog;

use qwiic_relay_rs::*;
use rppal::gpio::Gpio;

use std::error::Error;

use std::path::Path;

use std::thread;
use std::time::Duration;




use std::io::{self, BufRead};
use std::sync::mpsc::{self};


// Gpio uses BCM pin numbering. BCM GPIO 23 is tied to physical pin 16.
// const GPIO_LED: u8 = 23;

pub fn run(cmd: String) -> Result<(), Box<dyn Error>>{

    let command = cmd.clone();

    let split = command.split(' ');
    let _split_vec = split.collect::<Vec<&str>>();


    if command.starts_with("cls") || command.starts_with("clear"){
        aog::cls();
    }
    
    if command.starts_with("install"){
        // crate::setup::install();
    }

    if command.starts_with("uninstall"){
        crate::setup::uninstall();
    }
    
    if command.starts_with("stats"){
        aog::print_stats();
    }

    if command.starts_with("tvoc"){
        println!("{}", aog::sensors::get_value("tvoc"));
    }

    if command.starts_with("temp"){
        println!("{}", aog::sensors::get_value("temp"));
    }

    if command.starts_with("hum"){
        println!("{}", aog::sensors::get_value("hum"));
    }

    if command.starts_with("co2"){
        println!("{}", aog::sensors::get_value("co2"));
    }

    if command.starts_with("pm25"){
        println!("{}", aog::sensors::get_value("pm25"));
    }

    if command.starts_with("pm10"){
        println!("{}", aog::sensors::get_value("pm10"));
    }

    if command.starts_with("ph"){
        println!("{}", aog::sensors::get_value("ph"));
    }

    if command.starts_with("t1_ovf"){
        println!("{}", aog::sensors::get_value("t1_ovf"));
    }

    if command.starts_with("t2_ovf"){
        println!("{}", aog::sensors::get_value("t2_ovf"));
    }


    if command.starts_with("relay"){

        let qwiic_relay_config = QwiicRelayConfig::default();
        let mut qwiic_relay = QwiicRelay::new(qwiic_relay_config, "/dev/i2c-1", 0x25).expect("Could not init device");

     
    
        if command.contains("off"){
            if command.contains("2"){
                qwiic_relay.set_relay_off(Some(2)).unwrap();
            }
            if command.contains("3"){
                qwiic_relay.set_relay_off(Some(3)).unwrap();
            }
            if command.contains("4"){
                qwiic_relay.set_relay_off(Some(4)).unwrap();
            }
        }

        if command.contains("on"){
            if command.contains("2"){
                qwiic_relay.set_relay_on(Some(2)).unwrap();
            }
            if command.contains("3"){
                qwiic_relay.set_relay_on(Some(3)).unwrap();
            }
            if command.contains("4"){
                qwiic_relay.set_relay_on(Some(4)).unwrap();
            }
        }


    }

    
    if command.clone() == *"help"{
        println!("gpio status:                  prints status of the gpio bus");
        println!("gpio [on/off] [gpio_bdm]:     change state of a gpio pin");
        println!("clear/cls:                    clears screen");
        println!("help [command]:               shows help");
    }


    if command == *"test"{
        let selected_pin = 17;
        let mut pin = Gpio::new().unwrap().get(selected_pin).unwrap().into_output();
        loop {
            // let raw = aog::sensors::get_arduino_raw();

            // if raw.contains("TOP_TANK_OVERFLOW: NONE"){
            //     pin.set_low();
            // } else {
            //     pin.set_high();
            // }

        }
    }

    if command == *"stdout"{
        println!("{}", get_stdout().unwrap());
    }

    // 0-21
    if command.starts_with("gpio"){
        if command == *"gpio status"{
            let _ = aog::gpio::status::print();
        }

        if command.contains("on"){
            println!("Press enter to terminate the gpio on(set-low) thread");
            let (tx, _rx) = mpsc::channel();
            let split = cmd.split(' ');
            let split_vec = split.collect::<Vec<&str>>();
            let selected_pin = split_vec[2].parse::<u8>().unwrap();

            let gpio = Gpio::new();
            if gpio.is_ok() {
                let gpio_pin = gpio.unwrap().get(selected_pin);
                if gpio_pin.is_ok() {
                    let mut pin = gpio_pin.unwrap().into_output();
                    thread::spawn(move || loop {
                        pin.set_low();
                        thread::sleep(Duration::from_millis(500));
                    });
                } else {
                    log::warn!("Command '{}' failed. GPIO pin is unavailable.", command);
                }
            } else {
                log::warn!("Command '{}' failed. GPIO is unavailable.", command);
            }

        
            if !command.contains("nolock"){
                let mut line = String::new();
                let stdin = io::stdin();
                let _ = stdin.lock().read_line(&mut line);
                let _ = tx.send(());
            }

        } else if command.contains("off"){
            let split = cmd.split(' ');
            let split_vec = split.collect::<Vec<&str>>();
            let selected_pin = split_vec[2].parse::<u8>().unwrap();


            let gpio = Gpio::new();
            if gpio.is_ok() {
                let gpio_pin = gpio.unwrap().get(selected_pin);
                if gpio_pin.is_ok() {
                    gpio_pin.unwrap().into_input();
                } else {
                    log::warn!("Command '{}' failed. GPIO pin is unavailable.", command);
                }
            } else {
                log::warn!("Command '{}' failed. GPIO is unavailable.", command);
            }
            




        } else if command.contains("stress"){


            println!("Press enter to terminate the gpio stress thread");
            let (tx, _rx) = mpsc::channel();
            let split = cmd.split(' ');
            let split_vec = split.collect::<Vec<&str>>();
            let selected_pin = split_vec[2].parse::<u8>().unwrap();



            let gpio = Gpio::new();
            if gpio.is_ok() {
                let gpio_pin = gpio.unwrap().get(selected_pin);
                if gpio_pin.is_ok() {
                    let mut pin = gpio_pin.unwrap().into_output();
                    thread::spawn(move || loop {
                            pin.set_low();
                            thread::sleep(Duration::from_millis(2000));
                            pin.set_high();
                            thread::sleep(Duration::from_millis(2000));
                    });
                } else {
                    log::warn!("Command '{}' failed. GPIO pin is unavailable.", command);
                }
            } else {
                log::warn!("Command '{}' failed. GPIO is unavailable.", command);
            }


            if !command.contains("nolock"){
                let mut line = String::new();
                let stdin = io::stdin();
                let _ = stdin.lock().read_line(&mut line);
                let _ = tx.send(());
            }

 
        }
    }


    


    Ok(())
}

// use std::io::{self, Write};

fn get_stdout() -> io::Result<String> {
    let stdout = io::stdout();
    // let mut handle = stdout.lock();

    // handle.write_all(b"hello world")?;

    Ok(format!("{:?}", stdout))
}