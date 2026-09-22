use qwiic_lcd_rs::*;
use std::thread;
use std::time::Duration;
extern crate machine_ip;

use i2cdev::linux::LinuxI2CError;

const LCD_COLUMNS: usize = 20;
const CHARACTER_DELAY: Duration = Duration::from_millis(10);
const COMMAND_DELAY: Duration = Duration::from_millis(50);

pub fn init() {
    let mut ip = String::new();

    thread::spawn(move || loop {
        // log::info!("Updating LCD...");

        // Fetch IP Address
        let ipp = machine_ip::get();
        if ipp.is_some() {
            let tmpip = ipp.unwrap().to_string();
            if tmpip.len() > 0 {
                ip = tmpip;
            }
        }

        // Default LCDSize is 4x20
        let config = ScreenConfig::default();

        // Default Qwiic address is 0x72
        let screen = match Screen::new(config, "/dev/i2c-1", 0x72) {
            Ok(screen) => screen,
            Err(error) => {
                log::warn!("LCD unavailable: {}. Retrying in 10 seconds.", error);
                thread::sleep(Duration::from_secs(10));
                continue;
            }
        };

        // let arduino_raw = crate::aog::sensors::get_arduino_raw();
        let co2 = crate::aog::sensors::get_value("co2");
        let pm25 = crate::aog::sensors::get_value("pm25");

        let set_lcd_status = set_lcd(screen, ip.to_string(), co2, pm25);

        match set_lcd_status {
            Ok(_lcd_status) => {}
            Err(error) => {
                log::warn!("Failed to update LCD: {}. Retrying in 10 seconds.", error);
                thread::sleep(Duration::from_secs(10));
                continue;
            }
        }

        thread::sleep(Duration::from_secs(2));
    });
}

pub fn set_lcd(
    mut screen: Screen,
    ip: String,
    co2: String,
    pm25: String,
) -> Result<Screen, LinuxI2CError> {
    // The SerLCD needs time to process commands and characters. The dependency
    // waits only 10 microseconds, which overruns the connected display.
    screen.change_backlight(0, 255, 0)?;
    thread::sleep(COMMAND_DELAY);
    screen.write_special_cmd(Command::DisplayControl as u8 | DisplayStatus::On as u8)?;
    thread::sleep(COMMAND_DELAY);
    screen.write_special_cmd(Command::EntryModeSet as u8 | EntryMode::Left as u8)?;
    thread::sleep(COMMAND_DELAY);
    // Screen::clear() discards a write error and immediately sends another
    // command. Send clear directly so errors propagate and it has time to finish.
    screen.write_setting_cmd(Command::ClearDisplay as u8)?;
    thread::sleep(COMMAND_DELAY);

    let lines = [
        ip,
        format!("CO2: {}", co2),
        format!("PM2.5: {}", pm25),
        "Status: 001".into(),
    ];
    for (row, line) in lines.iter().enumerate() {
        screen.move_cursor(row, 0)?;
        thread::sleep(COMMAND_DELAY);
        for byte in lcd_line(line) {
            screen.write_byte(byte)?;
            thread::sleep(CHARACTER_DELAY);
        }
    }
    Ok(screen)
}

fn lcd_line(text: &str) -> [u8; LCD_COLUMNS] {
    let mut line = [b' '; LCD_COLUMNS];
    for (slot, character) in line.iter_mut().zip(text.chars()) {
        // Keep data from wrapping into another row or issuing SerLCD commands
        // (0x7c and 0xfe are command prefixes).
        *slot = if character.is_ascii() && !character.is_ascii_control() && character != '|' {
            character as u8
        } else {
            b'?'
        };
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_fit_the_display_without_wrapping() {
        assert_eq!(&lcd_line("192.168.86.133"), b"192.168.86.133      ");
        assert_eq!(
            &lcd_line("PM2.5: sensor unavailable"),
            b"PM2.5: sensor unavai"
        );
    }

    #[test]
    fn sensor_text_cannot_send_display_commands() {
        assert_eq!(&lcd_line("a|\u{fe}\n\u{80}z"), b"a????z              ");
    }
}
