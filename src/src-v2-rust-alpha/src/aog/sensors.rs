
use sds011::{SDS011};
use std::thread::sleep;


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


    return format!("");

}