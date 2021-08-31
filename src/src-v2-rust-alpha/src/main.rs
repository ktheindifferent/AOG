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


extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;


const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");


use signal_hook::consts::TERM_SIGNALS;


use simple_logger::SimpleLogger;



fn main() -> Result<(), std::io::Error> {

    // Setup a logfile if A.O.G. is installed. Clears old log on boot.
    // ----------------------------------------------------------------
    if Path::new("/opt/aog/").exists() {
        aog::init_log("/opt/aog/output.log".to_string());
        SimpleLogger::new().with_colors(true).with_output_file("/opt/aog/output.log".to_string()).init().unwrap();
    } else {
        SimpleLogger::new().with_colors(true).init().unwrap();
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


    // Init pump thread and start
    // ----------------------------------------------------------------
    let (tx, rx) = mpsc::channel();
    let mut pump_thread = aog::pump::PumpThread::default();
    pump_thread.tx = tx;
    // TODO - Check if this is disabled in the config first
    aog::pump::start(pump_thread.clone(), Arc::clone(&term_now), rx);


    // Init GPIO 27 thread and set low(relay-on)
    // ----------------------------------------------------------------
    let (tx_27_low, rx_27_low) = mpsc::channel();
    let (tx_27_high, _rx_27_high) = mpsc::channel();
    let mut gpio_27_thread = aog::gpio::thread::GPIOThread::default();
    gpio_27_thread.gpio_pin = 27;
    gpio_27_thread.set_low_tx = tx_27_low;
    gpio_27_thread.set_high_tx = tx_27_high;
    // TODO - Check if this is disabled in the config first
    aog::gpio::thread::set_low(gpio_27_thread.clone(), Arc::clone(&term_now), rx_27_low);

    // Init GPIO 22 thread and set low(relay-on)
    // ----------------------------------------------------------------
    let (tx_22_low, rx_22_low) = mpsc::channel();
    let (tx_22_high, _rx_22_high) = mpsc::channel();
    let mut gpio_22_thread = aog::gpio::thread::GPIOThread::default();
    gpio_22_thread.gpio_pin = 22;
    gpio_22_thread.set_low_tx = tx_22_low;
    gpio_27_thread.set_high_tx = tx_22_high;
    // TODO - Check if this is disabled in the config first
    aog::gpio::thread::set_low(gpio_22_thread.clone(), Arc::clone(&term_now), rx_22_low);

    // Collect command line arguments
    // ----------------------------------------------------------------
    


    // No ars triggers an interactive A.O.G. command line interface for debugging.
    // Any arguments means this is running in the background on the unit.
    // ----------------------------------------------------------------
    if env::args().count() > 1 {

        log::info!("Flags detected. A.O.G. is running as a background service.");

        if Path::new("/opt/aog/").exists() {

            aog::video::init_all();

            // Start Web Thread
            thread::spawn(|| {
                aog::web::init();
            });
        
            while !term_now.load(Ordering::Relaxed) {

            }
        } 
    } else {



        // This is a live terminal so clear the screen first.
        // ----------------------------------------------------------------
        aog::cls();

    
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

            // Start video thread(s)
            aog::video::init_all();

            // Start Web Thread
            thread::spawn(|| {
                aog::web::init();
            });

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

            // Pump Start Command
            // ----------------------------------------------------------------
            if s.clone() == "pump start"{
                let (tx, rx) = mpsc::channel();
                pump_thread.tx = tx;
                aog::pump::start(pump_thread.clone(), Arc::clone(&term_now), rx);
            } 

             // Pump Stop Command
             // ----------------------------------------------------------------
            if s.clone() == "pump stop"{
                aog::pump::stop(pump_thread.clone());
            }

            // Air Start Command
            // ----------------------------------------------------------------
            if s.clone() == "air start"{
                aog::gpio::thread::stop(gpio_27_thread.clone());
                let (tx_27_low, rx_27_low) = mpsc::channel();
                gpio_27_thread.set_low_tx = tx_27_low;
                aog::gpio::thread::set_low(gpio_27_thread.clone(), Arc::clone(&term_now), rx_27_low);
            }

            // Air Stop Command
            // ----------------------------------------------------------------
            if s.clone() == "air stop"{
                aog::gpio::thread::stop(gpio_27_thread.clone());
                let (tx_27_high, rx_27_high) = mpsc::channel();
                gpio_27_thread.set_high_tx = tx_27_high;
                aog::gpio::thread::set_high(gpio_27_thread.clone(), Arc::clone(&term_now), rx_27_high);
            }

            // Air Start Command
            // ----------------------------------------------------------------
            if s.clone() == "uv start"{
                aog::gpio::thread::stop(gpio_22_thread.clone());
                let (tx_22_low, rx_22_low) = mpsc::channel();
                gpio_22_thread.set_low_tx = tx_22_low;
                aog::gpio::thread::set_low(gpio_22_thread.clone(), Arc::clone(&term_now), rx_22_low);
            }

            // Air Stop Command
            // ----------------------------------------------------------------
            if s.clone() == "uv stop"{
                aog::gpio::thread::stop(gpio_22_thread.clone());
                let (tx_22_high, rx_22_high) = mpsc::channel();
                gpio_22_thread.set_high_tx = tx_22_high;
                aog::gpio::thread::set_high(gpio_22_thread.clone(), Arc::clone(&term_now), rx_22_high);
            }

            aog::command::run(s.clone());
        
    
    
        }



    }

    // Since our loop is basically an infinite loop,
    // that only ends when we receive SIGTERM, if
    // we got here, it's because the loop exited after
    // receiving SIGTERM
    println!("Exiting...");

    // Cleanup
    aog::pump::stop(pump_thread);
    aog::gpio::thread::stop(gpio_27_thread);
    aog::gpio::thread::stop(gpio_22_thread);

    Ok(())
}