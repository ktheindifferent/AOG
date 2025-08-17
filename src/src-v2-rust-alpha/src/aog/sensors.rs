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


use std::time::{Duration, SystemTime, UNIX_EPOCH};

use std::str;


use std::thread;
use std::sync::mpsc;


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

    let _ = thread::Builder::new().name("pm1025_thread".to_string()).spawn(move || loop {
        let pm10 = fetch_pm10();
        let pm25 = fetch_pm25();

        if pm10.len() > 0 {
            if let Ok(mut f) = File::create("/opt/aog/sensors/pm10") {
                let _ = f.write_all(pm10.as_bytes());
            }
        }

        if pm25.len() > 0 {
            if let Ok(mut f) = File::create("/opt/aog/sensors/pm25") {
                let _ = f.write_all(pm25.as_bytes());
            }
        }
        
        // Add sleep to prevent CPU spinning
        thread::sleep(Duration::from_secs(10));

    });




    let _ = thread::Builder::new().name("ovf_thread".to_string()).spawn(move || {

        fetch_arduino(format!("DUAL_OVF_SENSOR"));
        fetch_arduino(format!("SENSORKIT_MK1"));

 



    });
  
}

pub fn fetch_pm25() -> String {
    let mut tty_port = 0;
    let tty_quit = 10;
    while tty_port < tty_quit{
        match SDS011::new(format!("/dev/ttyUSB{}", tty_port).as_str()) {
            Ok(mut sensor) => {
                match sensor.set_work_period(10){
                    Ok(_wp) => {
                        if let Ok(m) = sensor.query() {
                            return format!("{}", m.pm25);
                        } else {
                            return String::new();
                        }
                    },
                    Err(_err) => {
                        
                    }
                }
            },
            Err(_e) => {
                tty_port += 1;
            }
        };
    }
    return String::new();
}

pub fn fetch_pm10() -> String {
    let mut tty_port = 0;
    let tty_quit = 10;
    while tty_port < tty_quit{
        match SDS011::new(format!("/dev/ttyUSB{}", tty_port).as_str()) {
            Ok(mut sensor) => {
                match sensor.set_work_period(10){
                    Ok(_wp) => {
                        if let Ok(m) = sensor.query() {
                            return format!("{}", m.pm10);
                        } else {
                            return String::new();
                        }
                    },
                    Err(_err) => {

                    }
                }
     
            },
            Err(_e) => {
                tty_port += 1
            }
        };
    }
    return String::new();
}





pub fn parse_arduino(raw: String, line_key: &str, on_fail_string: String) -> String {
    let split = raw.split('\n');
    let split_vec = split.collect::<Vec<&str>>();
    for line in split_vec {
        if line.contains(line_key) {
            let split2 = line.split(": ");
            let split2_vec = split2.collect::<Vec<&str>>();
            if split2_vec.len() > 1{
                return split2_vec[1].to_string();
            } else {
                return on_fail_string;
            }
        }
    }

    return on_fail_string;
}



pub fn get_value(sensor: &str) -> String {
    if Path::new(format!("/opt/aog/sensors/{}", sensor).as_str()).exists(){
        let mut data = String::new();
        match File::open(format!("/opt/aog/sensors/{}", sensor).as_str()) {
            Ok(mut f) => {
                let _ = f.read_to_string(&mut data);
                return data;
            },
            Err(_) => return format!("N/A"),
        }
    } else {
        return format!("N/A");
    }
}

use serial2::SerialPort;

// device_type: DUAL_OVF_SENSOR, SENSORKIT_MK1
pub fn fetch_arduino(device_type: String) {

 
    let _ = thread::Builder::new().name("fetch_arduino_thread".to_string()).spawn(move || {
        let mut tty_port = 0;
        let tty_quit = 10;
        let mut tty_found = false;

        // let ports = serialport::available_ports().expect("No ports found!");
        // for p in ports {
        //     log::info!("{}", p.port_name);
        // }


        while !tty_found{

            if tty_port > 10 {
                tty_port = 0;
            }
    
            let port_name = format!("/dev/ttyUSB{}", tty_port);

            if !std::path::Path::new(port_name.clone().as_str()).exists(){
                tty_port += 1;
            } else {
                let baud_rate = 9600;
                let mut ttsport = SerialPort::open(port_name.clone(), baud_rate);
    
            
                match ttsport {
                    Ok(mut port) => {
                        
                  
                            let mut serial_buf: Vec<u8> = vec![0; 256];
                            let mut response = String::new();
        
                            loop {
                                match port.read(serial_buf.as_mut_slice()) {
                                    Ok(t) => {

                                        let _ = std::io::stdout().write_all(&serial_buf[..t]);
                                        
                                        // RECOVERY: Clear error state if we successfully read from overflow sensor
                                        if device_type.contains("DUAL_OVF_SENSOR") && t > 0 {
                                            // Remove error state file to indicate recovery
                                            let _ = std::fs::remove_file("/opt/aog/sensors/overflow_error");
                                            
                                            // Log recovery only once per recovery
                                            if std::path::Path::new("/opt/aog/sensors/overflow_error").exists() {
                                                log::info!("Overflow sensor communication recovered at {}", port_name);
                                            }
                                        }
    
                                        // println!("found_arduino: {}", port_name.clone());
                        
                                        // let pre_value = str::from_utf8(&serial_buf[..t]);
            
                                        // if pre_value.is_ok(){
                                        //     let value = pre_value.unwrap().to_string();
                                        //     if !value.is_empty(){
                                        //         response += &value;
                                        //     }    
                                        // }

                                        // log::info!("response: {}", response.clone());

                                        // let parts = response.split("\n");
                                        // let collection = parts.collect::<Vec<&str>>();
                                        // log::info!("{:?}", collection);
                                        
                               
                                        // if collection.len() > 3 {
                                        //     if device_type.contains("SENSORKIT_MK1") && response.contains("BEGIN\nDEVICE_ID") && response.contains("\nEND\nBEGIN") && response.contains(device_type.as_str()) {
        
                                        //             let raw_arduino = response.clone();
            
                                        //             log::info!("Raw Arduino: {:?}", raw_arduino);
            
                                        //             // Parse co2 reading from arduino serial string
                                        //             let co2 = parse_arduino(raw_arduino.clone(), "CO2:", "".to_string());
                                                    
                                        //             if co2.len() > 0 {
                                        //                 let mut f = File::create("/opt/aog/sensors/co2").expect("Unable to create file");
                                        //                 f.write_all(co2.as_bytes()).expect("Unable to write data");
                                        //             }
                                            
                                        
                                        //             // Parse tvoc reading from arduino serial string
                                        //             let tvoc = parse_arduino(raw_arduino.clone(), "TVOC:", "".to_string());
                                        
                                        //             if tvoc.len() > 0 {
                                        //                 let mut f = File::create("/opt/aog/sensors/tvoc").expect("Unable to create file");
                                        //                 f.write_all(tvoc.as_bytes()).expect("Unable to write data");
                                        //             }
                                            
                                        
                                        //             // Parse temperature reading from arduino serial string
                                        //             let temp = parse_arduino(raw_arduino.clone(), "TEMP:", "".to_string());
                                        
                                        //             if temp.len() > 0 {
                                        //                 if !temp.contains("-"){
                                        //                     let mut f = File::create("/opt/aog/sensors/temp").expect("Unable to create file");
                                        //                     f.write_all(temp.as_bytes()).expect("Unable to write data");
                                        //                 }
                                        //             }
                                        
                                        //             // Parse humidity reading from arduino serial string
                                        //             let hum = parse_arduino(raw_arduino.clone(), "HUM:", "".to_string());
                                        
                                        //             if hum.len() > 0 {
                                        //                 if !hum.contains("-"){
                                        //                     let mut f = File::create("/opt/aog/sensors/hum").expect("Unable to create file");
                                        //                     f.write_all(hum.as_bytes()).expect("Unable to write data");
                                        //                 }
                                        //             }
        
                                        //             // response = String::new();

                                                
        
                                        //     } else if device_type.contains("DUAL_OVF_SENSOR") && response.contains("BEGIN\nDEVICE_ID") && response.contains("\nEND\nBEGIN") && response.contains(device_type.as_str()) {
                                                
                                        //         let raw_arduino_ovf = collection[collection.len() - 1].clone().to_string();
                                                
                                        //         log::info!("Raw Arduino OVF: {:?}", raw_arduino_ovf);
        
                                        //         let t1_ovf = parse_arduino(raw_arduino_ovf.clone(), "T1_OVF:", "OVERFLOW".to_string());
                                        //         if t1_ovf.len() > 0 {
                                        //             let mut f = File::create("/opt/aog/sensors/t1_ovf").expect("Unable to create file");
                                        //             f.write_all(t1_ovf.as_bytes()).expect("Unable to write data");
                                        //         }
                                    
                                        //         let t2_ovf = parse_arduino(raw_arduino_ovf.clone(), "T2_OVF:", "OVERFLOW".to_string());
                                        //         if t2_ovf.len() > 0 {
                                        //             let mut f = File::create("/opt/aog/sensors/t2_ovf").expect("Unable to create file");
                                        //             f.write_all(t2_ovf.as_bytes()).expect("Unable to write data");
                                        //         }
                                    
                                        //         let ph = parse_arduino(raw_arduino_ovf.clone(), "PH:", "".to_string());
                                        //         if ph.len() > 0 {
                                        //             let mut f = File::create("/opt/aog/sensors/ph").expect("Unable to create file");
                                        //             f.write_all(ph.as_bytes()).expect("Unable to write data");
                                        //         }
        
                                        //         // response = String::new();
                                    
                                        //     } else {
                                        //         if device_type.contains("DUAL_OVF_SENSOR") && response.contains("SENSORKIT_MK1"){
                                        //             // Wrong sensor, break loop
                                        //             tty_found = false;
                                        //             response = "".to_string();
                                        //             break;
                                        //         }
                                        //         if device_type.contains("SENSORKIT_MK1") && response.contains("DUAL_OVF_SENSOR"){
                                        //             // Wrong sensor, break loop
                                        //             tty_found = false;
                                        //             response = "".to_string();
                                        //             break;
                                        //         }
        
                                                
                                        //     }
                                        // }
                            
                                    },
                                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => (),
                                    Err(ref e) if e.kind() == std::io::ErrorKind::BrokenPipe => {

                                        // CRITICAL SAFETY: Set water level sensors to OVERFLOW as a precaution
                                        if device_type.contains("DUAL_OVF_SENSOR"){
                                            log::error!("CRITICAL: Serial communication failed for overflow sensors - setting to OVERFLOW state for safety");
                                            
                                            // Force both tank overflow sensors to OVERFLOW state
                                            if let Ok(mut f) = File::create("/opt/aog/sensors/t1_ovf") {
                                                let _ = f.write_all(b"OVERFLOW");
                                                log::warn!("Tank 1 overflow sensor set to OVERFLOW due to communication failure");
                                            }
                                            
                                            if let Ok(mut f) = File::create("/opt/aog/sensors/t2_ovf") {
                                                let _ = f.write_all(b"OVERFLOW");
                                                log::warn!("Tank 2 overflow sensor set to OVERFLOW due to communication failure");
                                            }
                                            
                                            // Write error state file for monitoring
                                            if let Ok(mut f) = File::create("/opt/aog/sensors/overflow_error") {
                                                let timestamp = SystemTime::now()
                                                    .duration_since(UNIX_EPOCH)
                                                    .unwrap_or_default()
                                                    .as_secs();
                                                let error_msg = format!("SENSOR_FAILURE: BrokenPipe at {} - timestamp: {}", 
                                                    port_name, timestamp);
                                                let _ = f.write_all(error_msg.as_bytes());
                                            }
                                        }


                                        tty_port = 0;  // Reset to 0 instead of -1 for u8 type
                                        break;
                                    },
                                    Err(e) => {
                                        log::error!("Serial read error: {:?}", e);
                                        // CRITICAL SAFETY: Set water level sensors to OVERFLOW as a precaution
                                        if device_type.contains("DUAL_OVF_SENSOR"){
                                            log::error!("CRITICAL: Serial read error for overflow sensors - setting to OVERFLOW state for safety");
                                            
                                            // Force both tank overflow sensors to OVERFLOW state
                                            if let Ok(mut f) = File::create("/opt/aog/sensors/t1_ovf") {
                                                let _ = f.write_all(b"OVERFLOW");
                                                log::warn!("Tank 1 overflow sensor set to OVERFLOW due to read error");
                                            }
                                            
                                            if let Ok(mut f) = File::create("/opt/aog/sensors/t2_ovf") {
                                                let _ = f.write_all(b"OVERFLOW");
                                                log::warn!("Tank 2 overflow sensor set to OVERFLOW due to read error");
                                            }
                                            
                                            // Write error state file for monitoring
                                            if let Ok(mut f) = File::create("/opt/aog/sensors/overflow_error") {
                                                let timestamp = SystemTime::now()
                                                    .duration_since(UNIX_EPOCH)
                                                    .unwrap_or_default()
                                                    .as_secs();
                                                let error_msg = format!("SENSOR_FAILURE: {} at {} - timestamp: {}", 
                                                    e, port_name, timestamp);
                                                let _ = f.write_all(error_msg.as_bytes());
                                            }
                                        }
                                    }
                                }
                   
                            }
                        
                    },
                    Err(ref e) => {
                        log::error!("{}", e);
                        // break;
                    }
        
                    
                }
    
                tty_port += 1;
            }


        }
    

    });



  

}


