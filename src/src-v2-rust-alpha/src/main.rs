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

pub mod setup;
pub mod aog;



use std::thread;


use std::io::{stdin,stdout,Write};

use std::path::{Path};






use std::env;

use signal_hook::flag;


use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use std::sync::mpsc::{self};

use std::sync::Mutex;

extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;

extern crate qwiic_lcd_rs;

use qwiic_lcd_rs::*;
// use std::thread;
use std::time::Duration;



const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");


use signal_hook::consts::TERM_SIGNALS;


use simple_logger::SimpleLogger;

use log::LevelFilter;

fn main() -> Result<(), std::io::Error> {

    // Setup a logfile if A.O.G. is installed. Clears old log on boot.
    // ----------------------------------------------------------------
    if Path::new("/opt/aog/").exists() {
        let init_log_status = aog::init_log("/opt/aog/output.log".to_string());
        if init_log_status.is_ok() {

            
            SimpleLogger::new().with_module_level("something", LevelFilter::Off).with_colors(true).with_output_file("/opt/aog/output.log".to_string()).init().unwrap();
        } else {
            SimpleLogger::new().with_module_level("something", LevelFilter::Off).with_colors(true).init().unwrap();
        }
    } else {
        SimpleLogger::new().with_module_level("something", LevelFilter::Off).with_colors(true).init().unwrap();
    }

    // Term now will be true when its time to terminate the software...
    // Ex. CTRL-C, SIGTERM
    // ----------------------------------------------------------------
    let term_now = Arc::new(AtomicBool::new(false));

    // Ask signal_hook to set the term variable to true
    // when the program receives a SIGTERM kill signal
    // ----------------------------------------------------------------
	for sig in TERM_SIGNALS {
        flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now))?;
        flag::register(*sig, Arc::clone(&term_now))?;
    }

    crate::aog::sensors::init();

    // Initialize the LCD
    crate::aog::lcd::init();


    // Init pump thread and start
    // ----------------------------------------------------------------
    // let (tx, rx) = mpsc::channel();
    // let mut pump_thread = Arc::new(Mutex::new(aog::pump::PumpThread::default()));
    // let mut pump_thread_lock = pump_thread.lock().unwrap();
    // pump_thread_lock.tx = tx;
    // std::mem::drop(pump_thread_lock);
    // // TODO - Check if this is disabled in the config first
    // aog::pump::start(Arc::clone(&pump_thread), Arc::clone(&term_now), rx);


    
    
    // // Air Pump Relay
    // // Init GPIO 27 thread and set low(relay-on)
    // // ----------------------------------------------------------------
    // let (tx_27_low, rx_27_low) = mpsc::channel();
    // let (tx_27_high, _rx_27_high) = mpsc::channel();
    // let mut gpio_27_thread = Arc::new(Mutex::new(aog::gpio::thread::GPIOThread::default()));

    // let mut gpio_27_thread_lock = gpio_27_thread.lock().unwrap();
    // gpio_27_thread_lock.gpio_pin = 27;
    // gpio_27_thread_lock.set_low_tx = tx_27_low;
    // gpio_27_thread_lock.set_high_tx = tx_27_high;
    // std::mem::drop(gpio_27_thread_lock);

    // // TODO - Check if this is disabled in the config first
    // aog::gpio::thread::set_low(Arc::clone(&gpio_27_thread), Arc::clone(&term_now), rx_27_low);


    // // UV Light Relay
    // // Init GPIO 22 thread and set low(relay-on)
    // // ----------------------------------------------------------------
    // let (tx_22_low, rx_22_low) = mpsc::channel();
    // let (tx_22_high, _rx_22_high) = mpsc::channel();
    // let mut gpio_22_thread = Arc::new(Mutex::new(aog::gpio::thread::GPIOThread::default()));
    // let mut gpio_22_thread_lock = gpio_22_thread.lock().unwrap();
    // gpio_22_thread_lock.gpio_pin = 22;
    // gpio_22_thread_lock.set_low_tx = tx_22_low;
    // gpio_22_thread_lock.set_high_tx = tx_22_high;
    // std::mem::drop(gpio_22_thread_lock);
    // // TODO - Check if this is disabled in the config first
    // aog::gpio::thread::set_low(Arc::clone(&gpio_22_thread), Arc::clone(&term_now), rx_22_low);


    // Start Web Thread
    // let uv_arc = Arc::clone(&gpio_22_thread);
    // let air_arc = Arc::clone(&gpio_27_thread);
    thread::spawn(|| {
        aog::web::init();
    });


    // let uv_arc2 = Arc::clone(&gpio_22_thread);
    // let air_arc2 = Arc::clone(&gpio_27_thread);
    let tn = Arc::clone(&term_now);
    thread::spawn(|| {
        aog::web::init_command_api(tn);
    });

    // Start video thread(s)
    aog::video::init_all();


    // No ars triggers an interactive A.O.G. command line interface for debugging.
    // Any arguments means this is running in the background on the unit.
    // ----------------------------------------------------------------
    if env::args().count() > 1 {

        log::info!("Flags detected. A.O.G. is running as a background service.");

        if Path::new("/opt/aog/").exists() {
            while !term_now.load(Ordering::Relaxed) {

            }
        } 
    } else {



        // This is a live terminal so clear the screen first.
        // ----------------------------------------------------------------
        // aog::cls();

    
        // Print banner
        // ----------------------------------------------------------------
        aog::print_stats();
    
    
        // If A.O.G. has never been installed ask user to install.
        // ----------------------------------------------------------------
        if !Path::new("/opt/aog/").exists() {
            setup::install();
        }
    
        // Checks if config file exist and is valid
        // Config can become invalid with software updates
        // ----------------------------------------------------------------
        if Path::new("/opt/aog/").exists() {
            let aog_config = load_file("/opt/aog/config.bin", 0);
    
            if aog_config.is_ok() {
                let cfg: aog::Config = aog_config.unwrap();
                if cfg.version_installed != *VERSION.unwrap_or("unknown"){
                    println!("An old A.O.G. install was detected.");
                    setup::update();
                } else {}
            } else {
                println!("A.O.G. config is corrupt....");
                println!("Deleting config and re-initializing setup...");
                setup::uninstall();
                setup::install();
            }
        }

        // A.O.G. Terminal Interface Loop
        // ----------------------------------------------------------------
        while !term_now.load(Ordering::Relaxed) {
            

            let mut s=String::new();
            print!("> ");
            let _=stdout().flush();
            stdin().read_line(&mut s).expect("Did not enter a correct string");
            if let Some('\n')=s.chars().next_back() {
                s.pop();
            } 
            if let Some('\r')=s.chars().next_back() {
                s.pop();
            } 

            // Note: Some commands need to be on the main loop for now.
            // ----------------------------------------------------------------



            // TODO
            // If localhost:9443 is available then notify that their is a running background instance.
            // Forward all commands to localhost:8443

            let params = [("input_command", s)];
            // let client = reqwest::Client::new();
            let der = std::fs::read("/opt/aog/crt/default/aog.local.der").unwrap();
            let cert = reqwest::Certificate::from_der(&der).unwrap();
    
            let res = reqwest::blocking::Client::builder()
            .add_root_certificate(cert)
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap()
            .post(format!("https://localhost:9443").as_str())
            .form(&params)
            .send()
            .unwrap();


            let body = res.text().unwrap();



            // // Pump Start Command
            // // ----------------------------------------------------------------
            // if s.clone() == "pump start"{
            //     let (tx, rx) = mpsc::channel();
            //     let mut pump_thread_lock = pump_thread.lock().unwrap();
            //     pump_thread_lock.tx = tx;
            //     std::mem::drop(pump_thread_lock);
            //     aog::pump::start(Arc::clone(&pump_thread), Arc::clone(&term_now), rx);
            // } 

            //  // Pump Stop Command
            //  // ----------------------------------------------------------------
            // if s.clone() == "pump stop"{
            //     aog::pump::stop(Arc::clone(&pump_thread));
            // }

            // // Air Start Command
            // // ----------------------------------------------------------------
            // if s.clone() == "air start"{
            //     aog::gpio::thread::stop(Arc::clone(&gpio_27_thread));
            //     let mut gpio_27_thread_lock = gpio_27_thread.lock().unwrap();
            //     let (tx_27_low, rx_27_low) = mpsc::channel();
            //     gpio_27_thread_lock.set_low_tx = tx_27_low;
            //     std::mem::drop(gpio_27_thread_lock);
            //     aog::gpio::thread::set_low(Arc::clone(&gpio_27_thread), Arc::clone(&term_now), rx_27_low);
            // }

            // // Air Stop Command
            // // ----------------------------------------------------------------
            // if s.clone() == "air stop"{
            //     aog::gpio::thread::stop(Arc::clone(&gpio_27_thread));
            //     let mut gpio_27_thread_lock = gpio_27_thread.lock().unwrap();
            //     let (tx_27_high, rx_27_high) = mpsc::channel();
            //     gpio_27_thread_lock.set_high_tx = tx_27_high;
            //     std::mem::drop(gpio_27_thread_lock);
            //     aog::gpio::thread::set_high(Arc::clone(&gpio_27_thread), Arc::clone(&term_now), rx_27_high);
            // }

            // // Air Start Command
            // // ----------------------------------------------------------------
            // if s.clone() == "uv start"{
            //     aog::gpio::thread::stop(Arc::clone(&gpio_22_thread));
            //     let mut gpio_22_thread_lock = gpio_22_thread.lock().unwrap();
            //     let (tx_22_low, rx_22_low) = mpsc::channel();
            //     gpio_22_thread_lock.set_low_tx = tx_22_low;
            //     std::mem::drop(gpio_22_thread_lock);
            //     aog::gpio::thread::set_low(Arc::clone(&gpio_22_thread), Arc::clone(&term_now), rx_22_low);
            // }

            // // Air Stop Command
            // // ----------------------------------------------------------------
            // if s.clone() == "uv stop"{
            //     aog::gpio::thread::stop(Arc::clone(&gpio_22_thread));
            //     let mut gpio_22_thread_lock = gpio_22_thread.lock().unwrap();
            //     let (tx_22_high, rx_22_high) = mpsc::channel();
            //     gpio_22_thread_lock.set_high_tx = tx_22_high;
            //     std::mem::drop(gpio_22_thread_lock);
            //     aog::gpio::thread::set_high(Arc::clone(&gpio_22_thread), Arc::clone(&term_now), rx_22_high);
            // }

            // let _ = aog::command::run(s.clone());
        
            
    
        }



    }

    // Since our loop is basically an infinite loop,
    // that only ends when we receive SIGTERM, if
    // we got here, it's because the loop exited after
    // receiving SIGTERM
    println!("Exiting...");

    // Cleanup
    // aog::pump::stop(Arc::clone(&pump_thread));
    // aog::gpio::thread::stop(Arc::clone(&gpio_27_thread));
    // aog::gpio::thread::stop(Arc::clone(&gpio_22_thread));

    Ok(())
}