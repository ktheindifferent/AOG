
use sds011::{SDS011};
use std::thread::sleep;

use std::io::{self, Write};
use std::time::Duration;

use std::str;


// S1CO2: 400.00ppm
// S1TVOC: 0ppb
// S2CO2: 1343.75ppm
// AVGCO2: 871.88ppm
// HUMIDITY: 43.00%
// TEMPERATURE: 29.00C  
// TOP_TANK_OVERFLOW: OVERFLOW
// BARREL_WATER_OVERFLOW: NONE

pub fn get_arduino_raw() -> String {
    let mut tty_port = 0;
    let mut tty_quit = 25;
    let mut tty_found = false;
    while tty_found == false && tty_port < tty_quit{

        let port_name = format!("/dev/ttyUSB{}", tty_port);
        let baud_rate = 9600;

        let port = serialport::new(port_name.clone(), baud_rate)
            .timeout(Duration::from_millis(10))
            .open();


        let mut response = String::new();

        match port {
            Ok(mut port) => {
                loop{
                    let mut serial_buf: Vec<u8> = vec![0; 1000];
                    match port.read(serial_buf.as_mut_slice()) {
                        Ok(t) => {

                            let value = str::from_utf8(&serial_buf[..t]).unwrap().to_string();
                            
                            if value.len() > 0{
                                response += &value;
                            }

                            if response.len() > 300 {
                                return response;
                            }
                            
                        },
                        Err(e) => {},
                    }
                }
            },
            Err(e) => {}

            
        }

        tty_port += 1;
    }

    return format!("N/A");

}

pub fn get_co2() -> String {
    let raw = get_arduino_raw();


    let split = raw.split("\n");
    let split_vec = split.collect::<Vec<&str>>();
    for line in split_vec {
        if line.contains("AVGCO2:") {
            let split2 = line.split(": ");
            let split2_vec = split2.collect::<Vec<&str>>();
            return split2_vec[1].to_string();
        }
    }

    return raw;
}


pub fn get_pm25() -> String {
    let mut tty_port = 0;
    let mut tty_quit = 25;
    let mut tty_found = false;
    while tty_found == false && tty_port < tty_quit{
        match SDS011::new(format!("/dev/ttyUSB{}", tty_port).as_str()) {
            Ok(mut sensor) => {
                sensor.set_work_period(5).unwrap();
                tty_found = true;
                if let Ok(m) = sensor.query() {
                    return format!("{}", m.pm25);
                } else {
                    return format!("");
                }
            },
            Err(e) => {
                tty_port += 1;
            }
        };
    }
    return format!("N/A");
}

pub fn get_pm10() -> String {
    let mut tty_port = 0;
    let mut tty_quit = 25;
    let mut tty_found = false;
    while tty_found == false && tty_port < tty_quit{
        match SDS011::new(format!("/dev/ttyUSB{}", tty_port).as_str()) {
            Ok(mut sensor) => {
                sensor.set_work_period(5).unwrap();
                tty_found = true;
                if let Ok(m) = sensor.query() {
                    return format!("{}", m.pm10);
                } else {
                    return format!("");
                }
            },
            Err(e) => {
                tty_port += 1
            }
        };
    }
    return format!("N/A");
}