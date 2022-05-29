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

use sds011::{SDS011};

use std::path::Path;


use std::time::Duration;

use std::str;


use std::thread;
use std::sync::mpsc;
use std::fs;


// TODO - ADD PH Sensor
// https://myhydropi.com/connecting-a-ph-sensor-to-a-raspberry-pi



// S1CO2: 400.00ppm
// S1TVOC: 0ppb
// S2CO2: 1343.75ppm
// AVGCO2: 871.88ppm
// HUMIDITY: 43.00%
// TEMPERATURE: 29.00C  
// TOP_TANK_OVERFLOW: OVERFLOW
// BARREL_WATER_OVERFLOW: NONE


use std::fs::File;
use std::io::Write;
use std::io::Read;

pub fn init(){

    thread::Builder::new().name("thread1".to_string()).spawn(move || loop {
        let raw_arduino = fetch_arduino();
        let pm10 = fetch_pm10();
        let pm25 = fetch_pm25();
        if raw_arduino.len() > 5{

            // Parse co2 reading from arduino serial string
            let co2 = parse_arduino_co2(raw_arduino.clone());

            if co2.len() > 0 {
                let mut f = File::create("/opt/aog/sensors/co2").expect("Unable to create file");
                f.write_all(co2.as_bytes()).expect("Unable to write data");
            }
    

            // Parse tvoc reading from arduino serial string
            let tvoc = parse_arduino_tvoc(raw_arduino.clone());

            if tvoc.len() > 0 {
                let mut f = File::create("/opt/aog/sensors/tvoc").expect("Unable to create file");
                f.write_all(tvoc.as_bytes()).expect("Unable to write data");
            }
    

            // Parse temperature reading from arduino serial string
            let temp = parse_arduino_temperature(raw_arduino.clone());

            if temp.len() > 0 {
                let mut f = File::create("/opt/aog/sensors/temp").expect("Unable to create file");
                f.write_all(temp.as_bytes()).expect("Unable to write data");
            }

            // Parse humidity reading from arduino serial string
            let hum = parse_arduino_humidity(raw_arduino.clone());

            if hum.len() > 0 {
                let mut f = File::create("/opt/aog/sensors/hum").expect("Unable to create file");
                f.write_all(hum.as_bytes()).expect("Unable to write data");
            }


        }


        if pm10.len() > 0 {
            let mut f = File::create("/opt/aog/sensors/pm10").expect("Unable to create file");
            f.write_all(pm10.as_bytes()).expect("Unable to write data");
        }

        if pm25.len() > 0 {
            let mut f = File::create("/opt/aog/sensors/pm25").expect("Unable to create file");
            f.write_all(pm25.as_bytes()).expect("Unable to write data");
        }

    });
  
}

pub fn fetch_pm25() -> String {
    let mut tty_port = 0;
    let tty_quit = 25;
    while tty_port < tty_quit{
        match SDS011::new(format!("/dev/ttyUSB{}", tty_port).as_str()) {
            Ok(mut sensor) => {
                sensor.set_work_period(5).unwrap();
                if let Ok(m) = sensor.query() {
                    return format!("{}", m.pm25);
                } else {
                    return format!("");
                }
            },
            Err(_e) => {
                tty_port += 1;
            }
        };
    }
    "".to_string()
}

pub fn fetch_pm10() -> String {
    let mut tty_port = 0;
    let tty_quit = 25;
    while tty_port < tty_quit{
        match SDS011::new(format!("/dev/ttyUSB{}", tty_port).as_str()) {
            Ok(mut sensor) => {
                sensor.set_work_period(5).unwrap();
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
    "".to_string()
}




pub fn parse_arduino_co2(raw: String) -> String {
    let split = raw.split('\n');
    let split_vec = split.collect::<Vec<&str>>();
    for line in split_vec {
        if line.contains("CO2:") {
            let split2 = line.split(": ");
            let split2_vec = split2.collect::<Vec<&str>>();
            return split2_vec[1].to_string();
        }
    }

    "".to_string()
}

pub fn parse_arduino_tvoc(raw: String) -> String {
    let split = raw.split('\n');
    let split_vec = split.collect::<Vec<&str>>();
    for line in split_vec {
        if line.contains("TVOC:") {
            let split2 = line.split(": ");
            let split2_vec = split2.collect::<Vec<&str>>();
            return split2_vec[1].to_string();
        }
    }

    "".to_string()
}

pub fn parse_arduino_temperature(raw: String) -> String {
    let split = raw.split('\n');
    let split_vec = split.collect::<Vec<&str>>();
    for line in split_vec {
        if line.contains("TEMP:") {
            let split2 = line.split(": ");
            let split2_vec = split2.collect::<Vec<&str>>();
            return split2_vec[1].to_string();
        }
    }

    "".to_string()
}

pub fn parse_arduino_humidity(raw: String) -> String {
    let split = raw.split('\n');
    let split_vec = split.collect::<Vec<&str>>();
    for line in split_vec {
        if line.contains("HUM:") {
            let split2 = line.split(": ");
            let split2_vec = split2.collect::<Vec<&str>>();
            return split2_vec[1].to_string();
        }
    }

    "".to_string()
}




pub fn fetch_arduino() -> String {

  
    let mut tty_port = 0;
    let tty_quit = 25;
    let mut tty_found = false;
    while !tty_found && tty_port < tty_quit{

        let port_name = format!("/dev/ttyUSB{}", tty_port);

        // log::info!("checking: {}", port_name.clone());

        let baud_rate = 9600;

        let port = serialport::new(port_name.clone(), baud_rate)
            .timeout(Duration::from_millis(100))
            .open();


        let mut response = String::new();

        match port {
            Ok(mut port) => {
                
    
                let mut serial_buf: Vec<u8> = vec![0; 1000];
                match port.read(serial_buf.as_mut_slice()) {
                    Ok(t) => {

                        // log::info!("found_arduino: {}", port_name.clone());
                        tty_found = true;

                        let pre_value = str::from_utf8(&serial_buf[..t]);

                        if pre_value.is_ok(){
                            let value = pre_value.unwrap().to_string();
                            if !value.is_empty(){
                                let value_cleaned = str::replace(&value, "\r", "");
                                response += &value_cleaned;
                            }    
                        }
                        
                        return response;
                

                        // log::info!("response: {}", response.clone());
                        
                        
                    },
                    Err(_e) => {
                        // break;
                    },
                }
            
            },
            Err(ref _e) => {
                // break;
            }

            
        }

        // std::mem::drop(port);

        tty_port += 1;
    }

    return "".to_string();
}

pub fn get_co2() -> String {
    if Path::new("/opt/aog/sensors/co2").exists(){
        let mut data = String::new();
        let mut f = File::open("/opt/aog/sensors/co2").expect("Unable to open file");
        f.read_to_string(&mut data).expect("Unable to read string");
        return data;
    } else {
        return format!("N/A");
    }
}

pub fn get_tvoc() -> String {
    if Path::new("/opt/aog/sensors/tvoc").exists(){
        let mut data = String::new();
        let mut f = File::open("/opt/aog/sensors/tvoc").expect("Unable to open file");
        f.read_to_string(&mut data).expect("Unable to read string");
        return data;
    } else {
        return format!("N/A");
    }
}

pub fn get_temperature() -> String {
    if Path::new("/opt/aog/sensors/temp").exists(){
        let mut data = String::new();
        let mut f = File::open("/opt/aog/sensors/temp").expect("Unable to open file");
        f.read_to_string(&mut data).expect("Unable to read string");
        return data;
    } else {
        return format!("N/A");
    }
}

pub fn get_humidity() -> String {
    if Path::new("/opt/aog/sensors/hum").exists(){
        let mut data = String::new();
        let mut f = File::open("/opt/aog/sensors/hum").expect("Unable to open file");
        f.read_to_string(&mut data).expect("Unable to read string");
        return data;
    } else {
        return format!("N/A");
    }
}



pub fn get_pm25() -> String {
    if Path::new("/opt/aog/sensors/pm25").exists(){
        let mut data = String::new();
        let mut f = File::open("/opt/aog/sensors/pm25").expect("Unable to open file");
        f.read_to_string(&mut data).expect("Unable to read string");
        return data;
    } else {
        return format!("N/A");
    }

}

pub fn get_pm10() -> String {
    if Path::new("/opt/aog/sensors/pm10").exists(){
        let mut data = String::new();
        let mut f = File::open("/opt/aog/sensors/pm10").expect("Unable to open file");
        f.read_to_string(&mut data).expect("Unable to read string");
        return data;
    } else {
        return format!("N/A");
    }
}

// pub fn get_arduino_raw() -> String {

//     let (sender, receiver) = mpsc::channel();
//     let _t = thread::spawn(move || {
//         let mut tty_port = 0;
//         let tty_quit = 25;
//         let mut tty_found = false;
//         while !tty_found && tty_port < tty_quit{
    
//             let port_name = format!("/dev/ttyUSB{}", tty_port);

//             println!("checking: {}", port_name.clone());

//             let baud_rate = 9600;
    
//             let port = serialport::new(port_name.clone(), baud_rate)
//                 .timeout(Duration::from_millis(100))
//                 .open();
    
    
//             let mut response = String::new();
    
//             match port {
//                 Ok(mut port) => {
                    
//                     loop{
//                         let mut serial_buf: Vec<u8> = vec![0; 1000];
//                         match port.read(serial_buf.as_mut_slice()) {
//                             Ok(t) => {

//                                 println!("found_arduino: {}", port_name.clone());
//                                 tty_found = true;
    
//                                 let pre_value = str::from_utf8(&serial_buf[..t]);
    
//                                 if pre_value.is_ok(){
//                                     let value = pre_value.unwrap().to_string();
//                                     if !value.is_empty(){
//                                         let value_cleaned = str::replace(&value, "\r", "");
//                                         response += &value_cleaned;
//                                     }    
//                                 }
                                
                         
//                                 if response.len() > 100 {
                           

//                                     match sender.send(response.clone()) {
//                                         Ok(()) => {}, // everything good
//                                         Err(_) => {}, // we have been released, don't panic
//                                     }

//                                     break;


//                                 }

//                                 println!("response: {}", response.clone());
                                
                                
//                             },
//                             Err(_e) => {
//                                 // break;
//                             },
//                         }
//                     }
//                 },
//                 Err(ref _e) => {
//                     // break;
//                 }
    
                
//             }

//             // std::mem::drop(port);
    
//             tty_port += 1;
//         }
    
//         "N/A".to_string()
//     });

//     let value = receiver.recv_timeout(Duration::from_millis(10000));

//     if value.is_ok(){
//         value.unwrap()
//     } else {
//         "N/A".to_string()
//     }


  

// }