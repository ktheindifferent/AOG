use qwiic_lcd_rs::*;
use std::thread;
use std::time::Duration;
extern crate machine_ip;

pub fn init(){

    thread::spawn(|| {

        // Default LCDSize is 4x20
        let mut config = ScreenConfig::default();

        // Uncomment and modify the values below to use different screen sizes
        // config.max_rows = 2;
        // config.max_columns = 16;
    
        // Default Qwiic address is 0x72
        let mut screen = Screen::new(config, "/dev/i2c-1", 0x72).expect("Could not init device");
    
        // Set backlight to green and wait 1 second
        screen.change_backlight(0, 255, 0).unwrap();
        thread::sleep(Duration::from_secs(1));
    
        // Set backlight to bright white
        screen.change_backlight(255, 255, 255).unwrap();

        loop {
            // Clear the screen
            screen.clear().unwrap();
            
            // Move the cursor to 0,0
            screen.move_cursor(0,0).unwrap();

            // Fetch IP Address
            let ip = machine_ip::get().unwrap();
        
            // Print text
            screen.print(format!("{}", ip.to_string()).as_str()).unwrap();
        
            // Move to the next line
            screen.move_cursor(1,0).unwrap();
        
            // Print text
            screen.print(format!("CO2: {}ppm", 0).as_str()).unwrap();

            // Move to the next line
            screen.move_cursor(2,0).unwrap();

            // Print text
            screen.print(format!("PM2.5: {}", 0).as_str()).unwrap();

            // Move to the next line
            screen.move_cursor(3,0).unwrap();

            // Print text
            screen.print(format!("Status: 001").as_str()).unwrap();

            // Sleep
            thread::sleep(Duration::from_millis(500));
        }
    


    });

}

