
use sds011::{SDS011};
use std::thread::sleep;

use std::io::{self, Write};
use std::time::Duration;


pub fn get_arduino_raw(){
    let port_name = "/dev/ttyUSB0";
    let baud_rate = 9600;

    let port = serialport::new(port_name, baud_rate)
        .timeout(Duration::from_millis(10))
        .open();

    match port {
        Ok(mut port) => {
            let mut serial_buf: Vec<u8> = vec![0; 1000];
            println!("Receiving data on {} at {} baud:", &port_name, &baud_rate);
            loop {
                match port.read(serial_buf.as_mut_slice()) {
                    Ok(t) => io::stdout().write_all(&serial_buf[..t]).unwrap(),
                    Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                    Err(e) => eprintln!("{:?}", e),
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to open \"{}\". Error: {}", port_name, e);
            ::std::process::exit(1);
        }
    }

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
                tty_port += 1
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
                    return format!("{}", m.pm25);
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