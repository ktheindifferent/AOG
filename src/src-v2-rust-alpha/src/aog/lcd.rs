use qwiic_lcd_rs::*;
use std::thread;
use std::time::Duration;
extern crate machine_ip;

use i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};

pub fn init(){


    let mut ip = String::new();



    thread::spawn(move || loop {

        // log::info!("Updating LCD...");

        // Fetch IP Address
        let ipp = machine_ip::get();
        if ipp.is_some(){
            let tmpip = ipp.unwrap().to_string();
            if tmpip.len() > 0{
                ip = tmpip;
            }
        }

        // Default LCDSize is 4x20
        let mut config = ScreenConfig::default();
    
        // Default Qwiic address is 0x72
        let mut screen = Screen::new(config, "/dev/i2c-1", 0x72).expect("Could not init device");

        // let arduino_raw = crate::aog::sensors::get_arduino_raw();
        let co2 = crate::aog::sensors::get_value("co2");
        let pm25 = crate::aog::sensors::get_value("pm25");

        let set_lcd_status = set_lcd(screen, ip.to_string(), co2, pm25);

        
        match set_lcd_status {
            Ok(lcd_status) => {
                
            },
            Err(err) => {
                // log::error!("{}", err);
            }
        }

        thread::sleep(Duration::from_secs(2));

        

    });

}

pub fn set_lcd(mut screen: Screen, ip: String, co2: String, pm25: String) -> Result<Screen, LinuxI2CError>{
    // Set backlight to green and wait 1 second
    screen.change_backlight(0, 255, 0)?;
    // thread::sleep(Duration::from_secs(1));

    // Set backlight to bright white
    // screen.change_backlight(255, 255, 255)?;

    // Clear the screen
    screen.clear()?;
        
    // Move the cursor to 0,0
    screen.move_cursor(0,0)?;

    // Print text
    screen.print(format!("{}", ip).as_str())?;

    // Move to the next line
    screen.move_cursor(1,0)?;

 
    // Print text
    screen.print(format!("CO2: {}", co2).as_str())?;

    // Move to the next line
    screen.move_cursor(2,0)?;

    // Print text
    screen.print(format!("PM2.5: {}", pm25).as_str())?;

    // Move to the next line
    screen.move_cursor(3,0)?;

    // Print text
    screen.print(format!("Status: 001").as_str())?;


    return Ok(screen);
}