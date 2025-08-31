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

pub mod command;
pub mod command_exec;
pub mod sensors;
pub mod gpio;
pub mod lcd;
pub mod video;
pub mod pump;
pub mod pump_safety;
pub mod water_level;
pub mod http;
pub mod tools;
pub mod error;
pub mod retry;
pub mod error_monitor;
pub mod qwiic;
pub mod auth;
pub mod instance;

#[cfg(test)]
mod tests_overflow;


use std::io::{Write, stdout};

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");





use shuteye::sleep;



use std::time::Duration;



use std::fs::File;







// const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

pub fn init_log(path: String) -> Result<(), std::io::Error>{
    let mut output = File::create(path.as_str())?;
    write!(output, "")?;
    Ok(())
}

pub fn sensors_check_animation(){
    let mut stdout = stdout();

    for i in 0..=99 {
        print!("\rChecking sensors {}%...", i);
        // or
        // stdout.write(format!("\rProcessing {}%...", i).as_bytes()).unwrap();

        let _ = stdout.flush();
        sleep(Duration::from_millis(20));
    }
    println!();
}


pub fn print_stats(){
    println!();
    sensors_check_animation();
    println!();

    // let arduino_raw = sensors::get_arduino_raw();

    println!("           PM2.5:    {}", sensors::get_value("pm25"));
    println!("            PM10:    {}", sensors::get_value("pm_10"));
    println!("             CO2:    {}", sensors::get_value("co2"));
    println!("            TVOC:    {}", sensors::get_value("tvoc")); 
    println!("        HUMIDITY:    {}", sensors::get_value("hum")); 
    println!("     TEMPERATURE:    {}", sensors::get_value("temp"));
    println!("              PH:    {}", sensors::get_value("ph"));
    println!("          T1_OVF:    {}", sensors::get_value("t1_ovf"));
    println!("          T2_OVF:    {}", sensors::get_value("t2_ovf"));
    
    println!();
}

pub fn print_logo(){
    println!("\n\n");
    println!(r"█████      ██████      ██████      ");
    println!(r"██   ██    ██    ██    ██          ");
    println!(r"███████    ██    ██    ██   ███    ");
    println!(r"██   ██    ██    ██    ██    ██    ");
    println!(r"██   ██ ██  ██████  ██  ██████  ██ ");                      
    println!(r"----------------------------------------------------------------------------");
    println!("v{}-alpha", VERSION.unwrap_or("UNKNOWN"));
    println!(r"----------------------------------------------------------------------------");
}

pub fn cls(){
    let _ = std::process::Command::new("cls").status()
        .or_else(|_| std::process::Command::new("clear").status())
        .map(|status| status.success())
        .unwrap_or(false);
    print_logo();
}



// pub fn load_config() -> Result<Config, SavefileError> {

//     if !Path::new("/opt/aog/config.bin").exists() {
//         let server_data = Config::default();
//         save_file("/opt/aog/config.bin", 0, &server_data).unwrap();
//     }

//     let result = load_file("/opt/aog/config.bin", 0);
//     if result.is_ok(){
//         Ok(result.unwrap())
//     } else {
//         let mut rng = rand::thread_rng();
//         let n1: u8 = rng.gen();
//         sleep(Duration::from_millis(n1.into()));
//         load_config()
//     }
// }

// pub fn load_sessions(config: Arc<Mutex<Config>>) -> Result<Sessions, SavefileError> {
//     // Result<T, SavefileError>
//     let result = load_file("/opt/aog/dat/sessions.bin", 0);
//     if format!("{:?}", result).contains("Err("){
//         let mut rng = rand::thread_rng();
//         let n1: u8 = rng.gen();
//         sleep(Duration::from_millis(n1.into()));
//         println!("error loading session file -- trying again...{}", format!("{:?}", result));
//         // Err(result.unwrap())
//         load_sessions(Arc::clone(&config))
//     } else {
//         Ok(result.unwrap())
//     }

// }