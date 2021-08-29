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

use signal_hook::flag;


use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use std::sync::mpsc::{self, TryRecvError};


extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;


const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");


use signal_hook::consts::TERM_SIGNALS;


use simple_logger::SimpleLogger;
// use chrono::Local;
// use env_logger::Builder;
// use log::LevelFilter;


fn main() -> Result<(), std::io::Error> {


    // Builder::new()
    //     .format(|buf, record| {
    //         writeln!(buf,
    //             "{} [{}] - {}",
    //             Local::now().format("%Y-%m-%dT%H:%M:%S"),
    //             record.level(),
    //             record.args()
    //         )
    //         println!("")
    //     })
    //     .filter(None, LevelFilter::Info)
    //     .init();

    aog::init_log(format!("/opt/aog/output.log"));
    SimpleLogger::new().with_colors(true).with_output_file(format!("/opt/aog/output.log")).init().unwrap();






    // Declare bool, setting it to false
    let term_now = Arc::new(AtomicBool::new(false));

    // Ask signal_hook to set the term variable to true
    // when the program receives a SIGTERM kill signal
	for sig in TERM_SIGNALS {
        flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now))?;
        flag::register(*sig, Arc::clone(&term_now))?;
    }



    let (tx, rx) = mpsc::channel();
    let mut pump_thread = aog::pump::PumpThread::default();
    pump_thread.tx = tx;
    // aog::pump::start(pump_thread);
    aog::pump::start(pump_thread.clone(), Arc::clone(&term_now), rx);

    let args: Vec<String> = env::args().collect();


    if args.len() > 1 {

        log::info!("Flags detected. A.O.G. is running as a background service.");
        

        // Secondary-Tank Water Pump Thread
        // TODO - Check if this is disabled in the config first
        // aog::command::run("top_tank_pump_start".to_string());
        aog::command::run("gpio on 27 nolock".to_string());
        aog::command::run("gpio on 22 nolock".to_string());
        

        if Path::new("/opt/aog/").exists() {

            // Start video0 Thread
            thread::spawn(|| {
                aog::video::init(format!("video0"));
            });

        

        
                    // Secondary-Tank Water Pump Thread
                    // TODO - Check if this is disabled in the config first
                    // aog::command::run("top_tank_pump_start".to_string());
                    // aog::command::run("gpio on 27".to_string());
                    // aog::command::run("gpio on 22".to_string());
                    
                // Start video2 Thread
                thread::spawn(|| {
                    aog::video::init(format!("video2"));
                });

                // Start Web Thread
                thread::spawn(|| {
                    aog::web::init();
                });
            
         

            while !term_now.load(Ordering::Relaxed) {

            }
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
                } else {}
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

        while !term_now.load(Ordering::Relaxed) {

            let mut s=String::new();
            println!("");
            print!("> ");
            let _=stdout().flush();
            stdin().read_line(&mut s).expect("Did not enter a correct string");
            if let Some('\n')=s.chars().next_back() {
                s.pop();
            } else {}
            if let Some('\r')=s.chars().next_back() {
                s.pop();
            } else {}
    

            if s.clone() == "pump start"{
                let (tx, rx) = mpsc::channel();
                pump_thread.tx = tx;
                aog::pump::start(pump_thread.clone(), Arc::clone(&term_now), rx);
            } else {}

            if s.clone() == "pump stop"{
                aog::pump::stop(pump_thread.clone());
            } else {}

            aog::command::run(s.clone());
        
    
    
        }



    }
    
    

    // Since our loop is basically an infinite loop,
    // that only ends when we receive SIGTERM, if
    // we got here, it's because the loop exited after
    // receiving SIGTERM
    println!("Exiting...");

    // Cleanup
    aog::pump::stop(pump_thread.clone());

    return Ok(());








    




    
    
  







}