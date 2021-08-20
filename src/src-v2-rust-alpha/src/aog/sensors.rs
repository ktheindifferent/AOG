
use sds011::{SDS011};
use std::thread::sleep;

use std::io::{self, Write};
use std::time::Duration;

use std::str;

use std::net;
use std::thread;
use std::sync::mpsc;

// S1CO2: 400.00ppm
// S1TVOC: 0ppb
// S2CO2: 1343.75ppm
// AVGCO2: 871.88ppm
// HUMIDITY: 43.00%
// TEMPERATURE: 29.00C  
// TOP_TANK_OVERFLOW: OVERFLOW
// BARREL_WATER_OVERFLOW: NONE


pub fn get_arduino_raw() -> String {

    let (sender, receiver) = mpsc::channel();
    let t = thread::spawn(move || {
        let mut tty_port = 0;
        let mut tty_quit = 25;
        let mut tty_found = false;
        while tty_found == false && tty_port < tty_quit{
    
            let port_name = format!("/dev/ttyUSB{}", tty_port);

            println!("checking: {}", port_name.clone());

            let baud_rate = 9600;
    
            let port = serialport::new(port_name.clone(), baud_rate)
                .timeout(Duration::from_millis(100))
                .open();
    
    
            let mut response = String::new();
    
            match port {
                Ok(mut port) => {
                    
                    loop{
                        let mut serial_buf: Vec<u8> = vec![0; 1000];
                        match port.read(serial_buf.as_mut_slice()) {
                            Ok(t) => {

                                println!("found_arduino: {}", port_name.clone());
                                tty_found = true;
    
                                let pre_value = str::from_utf8(&serial_buf[..t]);
    
                                if pre_value.is_ok(){
                                    let value = pre_value.unwrap().to_string();
                                    if value.len() > 0{
                                        response += &value;
                                    }    
                                }
                                
                         
                                if response.len() > 500 {
                                    return response;
                                    break;
                                }
                                
                                
                            },
                            Err(e) => {},
                        }
                    }
                },
                Err(ref e) => {}
    
                
            }

            std::mem::drop(port);
    
            tty_port += 1;
        }
    
        return format!("N/A");
    });

    let value = receiver.recv_timeout(Duration::from_millis(5000));

    if value.is_ok(){
        return value.unwrap();
    } else {
        return format!("N/A");
    }


  

}

pub fn get_co2(raw: String) -> String {
    let split = raw.split("\n");
    let split_vec = split.collect::<Vec<&str>>();
    for line in split_vec {
        if line.contains("AVGCO2:") {
            let split2 = line.split(":");
            let split2_vec = split2.collect::<Vec<&str>>();
            return split2_vec[1].to_string();
        }
    }

    return "N/A".to_string();
}

pub fn get_tvoc(raw: String) -> String {
    let split = raw.split("\n");
    let split_vec = split.collect::<Vec<&str>>();
    for line in split_vec {
        if line.contains("S1TVOC:") {
            let split2 = line.split(":");
            let split2_vec = split2.collect::<Vec<&str>>();
            return split2_vec[1].to_string();
        }
    }

    return "N/A".to_string();
}

pub fn get_temperature(raw: String) -> String {
    let split = raw.split("\n");
    let split_vec = split.collect::<Vec<&str>>();
    for line in split_vec {
        if line.contains("TEMPERATURE:") {
            let split2 = line.split(":");
            let split2_vec = split2.collect::<Vec<&str>>();
            return split2_vec[1].to_string();
        }
    }

    return "N/A".to_string();
}

pub fn get_humidity(raw: String) -> String {
    let split = raw.split("\n");
    let split_vec = split.collect::<Vec<&str>>();
    for line in split_vec {
        if line.contains("HUMIDITY:") {
            let split2 = line.split(":");
            let split2_vec = split2.collect::<Vec<&str>>();
            return split2_vec[1].to_string();
        }
    }

    return "N/A".to_string();
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