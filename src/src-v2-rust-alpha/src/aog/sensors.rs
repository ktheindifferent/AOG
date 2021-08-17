
use sds011::{SDS011};
use std::thread::sleep;

use std::io::{self, Write};
use std::time::Duration;

use std::str;

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

        match port {
            Ok(mut port) => {
                let mut serial_buf: Vec<u8> = vec![0; 10000];
                match port.read(serial_buf.as_mut_slice()) {
                    Ok(t) => {
                        io::stdout().write_all(&serial_buf[..t]).unwrap();
                        return str::from_utf8(&serial_buf[..t]).unwrap().to_string();
                    },
                    Err(e) => println!(""),
                }
            },
            Err(e) => {
                eprintln!("Failed to open \"{}\". Error: {}", port_name, e);
            }

            
        }

        tty_port += 1;
    }

    return format!("N/A");

}

pub fn get_co2() -> String {
    let raw = get_arduino_raw();

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