pub mod setup;
pub mod aog;


use std::error::Error;
use std::thread;
use std::time::Duration;

use std::io::{stdin,stdout,Write};

use std::path::{Path};

use rppal::gpio::Gpio;

use serde::{Serialize, Deserialize};
use shuteye::sleep;

use std::env;


use std::sync::mpsc::{self, TryRecvError};


extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;


const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

// TODO - handle SIGTERM by killing all threads and cleaning up gpio bus
// https://stackoverflow.com/questions/26199926/how-to-terminate-or-suspend-a-rust-thread-from-another-thread

fn main() {


    let (tx, rx) = mpsc::channel();
    let mut pump_thread = aog::pump::PumpThread::default();
    pump_thread.tx = tx;
    // aog::pump::start(pump_thread);
    aog::pump::start(pump_thread.clone(), rx);

    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        

        // Secondary-Tank Water Pump Thread
        // TODO - Check if this is disabled in the config first
        // aog::command::run("top_tank_pump_start".to_string());
        // aog::command::run("gpio on 27".to_string());
        // aog::command::run("gpio on 22".to_string());
        

        if Path::new("/opt/aog/").exists() {

            // Start video0 Thread
            thread::spawn(|| {
                aog::video::init(format!("video0"));
            });

            // Start video1 Thread
            thread::spawn(|| {
                aog::video::init(format!("video1"));
            });

            // Start video2 Thread
            thread::spawn(|| {
                aog::video::init(format!("video2"));
            });

            // Start Web Thread
            thread::spawn(|| {
                aog::web::init();
            });
        
        }

        loop {

        }
    } else {

        // If no args are found assume this is an interactive console




        aog::cls();

        


        // Running on screen
        aog::print_stats();
    
    
        if !Path::new("/opt/aog/").exists() {
            setup::install();
        }
    
    
        // Does config file exist and is it valid?
        // Config can become invalid with software updates
        if Path::new("/opt/aog/").exists() {
            let aog_config = load_file("/opt/aog/config.bin", 0);
    
            if aog_config.is_ok() {
                let cfg: aog::Config = aog_config.unwrap();
                if cfg.version_installed != VERSION.unwrap_or("unknown").to_string(){
                    println!("An old A.O.G. install was detected.");
                    setup::update();
                }
            } else {
                println!("A.O.G. config is corrupt....");
                println!("Deleting config and re-initializing setup...");
                setup::uninstall();
                setup::install();
            }

            // Start video0 Thread
            thread::spawn(|| {
                aog::video::init(format!("video0"));
            });

            // Start video1 Thread
            thread::spawn(|| {
                aog::video::init(format!("video1"));
            });

            // Start video2 Thread
            thread::spawn(|| {
                aog::video::init(format!("video2"));
            });

            // Start Web Thread
            thread::spawn(|| {
                aog::web::init();
            });

        }

        loop {

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
    

            if s.clone() == "pump start"{
                let (tx, rx) = mpsc::channel();
                pump_thread.tx = tx;
                aog::pump::start(pump_thread.clone(), rx);
            }

            if s.clone() == "pump stop"{
                aog::pump::stop(pump_thread.clone());
            }

            aog::command::run(s.clone());
        
    
    
        }



    }






    




    
    
  







}