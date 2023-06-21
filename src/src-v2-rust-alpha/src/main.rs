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

// Relay 1: Lights + Air
// Relay 2: Drain
// Relay 3: Fill
// Relay 4: Aux Tank Pump

pub mod setup;
pub mod aog;

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
extern crate qwiic_lcd_rs;
extern crate qwiic_relay_rs;

use clap::Parser;
use qwiic_lcd_rs::*;
use qwiic_relay_rs::*;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;
use simple_logger::SimpleLogger;
use std::env;
use std::io::{stdin,stdout,Write};
use std::path::{Path};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), std::io::Error> {

    let args = AOG::Args::parse();

    setup::install(args.clone());

    let config = Arc::new(Mutex::new(AOG::Config::load(0).unwrap()));

    crate::aog::sensors::init();

    // Initialize the LCD
    crate::aog::lcd::init();

    // Initialize the log system
    aog::init_log("/opt/aog/output.log".to_string()).unwrap();
    SimpleLogger::new().with_colors(true).with_output_file("/opt/aog/output.log".to_string()).init().unwrap();


    // Turn off all relays
    let qwiic_device = crate::aog::qwiic::QwiicRelayDevice::new(0x25);
    qwiic_device.test();






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



    thread::spawn(|| {
        aog::http::init();
    });



    thread::spawn(|| {
        aog::http::init_command_api();
    });

    // Start video thread(s)
    aog::video::init_all();

    // Print banner
    // ----------------------------------------------------------------
    aog::print_stats();


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