use std::error::Error;
use std::thread;
use std::time::Duration;

use std::io::{stdin,stdout,Write};

use rppal::gpio::Gpio;

use serde::{Serialize, Deserialize};
use shuteye::sleep;

extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;


// Gpio uses BCM pin numbering. BCM GPIO 23 is tied to physical pin 16.
const GPIO_LED: u8 = 23;


#[derive(Serialize, Deserialize, Savefile, Debug, Clone)]
pub struct SensorKitLog {
    pub id: String,
    pub timestamp: usize,
    pub s1_co2: String,
    pub s2_co2: String,
    pub avg_co2: String,
    pub humidity: String,
    pub temperature: String,
    pub is_tank_one_overflowed: bool,
    pub is_tank_two_overflowed: bool
}



fn main() -> Result<(), Box<dyn Error>> {

    println!("\n\n");
    println!(r"█████      ██████      ██████      ");
    println!(r"██   ██    ██    ██    ██          ");
    println!(r"███████    ██    ██    ██   ███    ");
    println!(r"██   ██    ██    ██    ██    ██    ");
    println!(r"██   ██ ██  ██████  ██  ██████  ██ ");                      
    println!(r"------------------------------------------------------------------");
    println!(r"v2.0.0-alpha");
    println!(r"------------------------------------------------------------------");
    println!("\n\n");




    // Retrieve the GPIO pin and configure it as an output.
    let mut pin = Gpio::new()?.get(GPIO_LED)?.into_output();

    loop {
        pin.toggle();
        thread::sleep(Duration::from_millis(500));
    }

    let mut s=String::new();
    print!("Please enter some text: ");
    let _=stdout().flush();
    stdin().read_line(&mut s).expect("Did not enter a correct string");
    if let Some('\n')=s.chars().next_back() {
        s.pop();
    }
    if let Some('\r')=s.chars().next_back() {
        s.pop();
    }
    println!("You typed: {}",s);



}