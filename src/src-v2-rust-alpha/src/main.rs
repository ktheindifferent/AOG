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

extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;


// Gpio uses BCM pin numbering. BCM GPIO 23 is tied to physical pin 16.
const GPIO_LED: u8 = 23;





fn main() {

    aog::cls();




    if !Path::new("/opt/aog/dat/").exists() {
		setup::install();
	} else {
        // TODO - Print installed version and check for updates

    }

    // Retrieve the GPIO pin and configure it as an output.
    // let mut pin = Gpio::new()?.get(GPIO_LED)?.into_output();

    loop {
        // pin.toggle();
        // thread::sleep(Duration::from_millis(500));

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

        aog::command::run(s);
        

        // if s.contains("Y") || s.contains("y") {
        //     aog_config.power_type = "solar";
        // } else {
        //     aog_config.power_type = "grid";
        // }


    }




}