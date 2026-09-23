use qwiic_lcd_rs::*;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use i2cdev::linux::LinuxI2CError;

const LCD_COLUMNS: usize = 20;
const LCD_ROWS: usize = 4;
const STATUS_COLUMNS: usize = LCD_COLUMNS - "Status: ".len();
const CHARACTER_DELAY: Duration = Duration::from_millis(10);
const COMMAND_DELAY: Duration = Duration::from_millis(50);
const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const IP_INTERVAL: Duration = Duration::from_secs(30);
const RETRY_INTERVAL: Duration = Duration::from_secs(10);
const SCROLL_INTERVAL: Duration = Duration::from_millis(450);
const SCROLL_HOLD: Duration = Duration::from_secs(2);

pub fn init() {
    thread::spawn(|| {
        let directory = Path::new("/opt/aog");
        let started = Instant::now();
        let mut next_sample = started;
        let mut next_ip = started;
        let mut retry_at = started;
        let mut ip = None;
        let mut display: Option<LcdDisplay> = None;
        let mut controller = None;
        let mut ticker = StatusTicker::new(started);
        let mut lines = [[b' '; LCD_COLUMNS]; LCD_ROWS];

        loop {
            let now = Instant::now();
            if now >= next_ip {
                ip = machine_ip::get().map(|address| address.to_string());
                lines[0] = lcd_line(ip.as_deref().unwrap_or("IP: unavailable"));
                next_ip = now + IP_INTERVAL;
            }
            if now >= next_sample {
                let readings = super::sensor_history::current_readings(directory);
                let value = |key| {
                    readings
                        .iter()
                        .find(|reading| reading.0 == key)
                        .and_then(|reading| reading.1)
                };
                lines[1] = lcd_line(&match value("co2") {
                    Some(value) => format!("CO2: {:.0}ppm", value),
                    None => "CO2: unavailable".into(),
                });
                lines[2] = lcd_line(&match value("pm25") {
                    Some(value) => format!("PM2.5: {:.1} ug/m3", value),
                    None => "PM2.5: unavailable".into(),
                });

                // A busy controller is normal during a command. Keep its last
                // observation briefly, without blocking output control.
                if let Some(status) = super::web_control::current_status() {
                    controller = Some((status, now));
                }
                let controller_status = controller
                    .as_ref()
                    .filter(|(_, sampled)| now.duration_since(*sampled) < RETRY_INTERVAL)
                    .map(|(status, _)| status);
                let status = health_status(
                    directory,
                    &readings,
                    controller_status,
                    ip.is_some(),
                    now.duration_since(started) < Duration::from_secs(30),
                );
                ticker.set_status(status, now);
                next_sample = now + SAMPLE_INTERVAL;
            }
            lines[3] = ticker.line(now);

            if display.is_none() && now >= retry_at {
                match LcdDisplay::connect() {
                    Ok(connected) => {
                        display = Some(connected);
                        log::info!("LCD connected; updating changed characters only");
                    }
                    Err(error) => {
                        log::warn!("LCD unavailable: {}. Retrying in 10 seconds.", error);
                        retry_at = Instant::now() + RETRY_INTERVAL;
                    }
                }
            }
            if let Some(connected) = display.as_mut() {
                if let Err(error) = connected.update(&lines, ticker.status.level.color()) {
                    log::warn!("Failed to update LCD: {}. Retrying in 10 seconds.", error);
                    // A partial write makes the cached frame untrustworthy.
                    // Reinitialize and repaint once the connection recovers.
                    display = None;
                    retry_at = Instant::now() + RETRY_INTERVAL;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    });
}

struct LcdDisplay {
    screen: Screen,
    frame: [[u8; LCD_COLUMNS]; LCD_ROWS],
    backlight: Option<[u8; 3]>,
}

impl LcdDisplay {
    fn connect() -> Result<Self, LinuxI2CError> {
        let mut screen = Screen::new(ScreenConfig::default(), "/dev/i2c-1", 0x72)?;
        // The SerLCD needs time to process commands and characters. The
        // dependency's 10 microsecond delay overruns this display.
        screen.write_special_cmd(Command::DisplayControl as u8 | DisplayStatus::On as u8)?;
        thread::sleep(COMMAND_DELAY);
        screen.write_special_cmd(Command::EntryModeSet as u8 | EntryMode::Left as u8)?;
        thread::sleep(COMMAND_DELAY);
        // Clear only at connection time. Screen::clear() discards a write
        // error, so issue the command directly and propagate failures.
        screen.write_setting_cmd(Command::ClearDisplay as u8)?;
        thread::sleep(COMMAND_DELAY);
        Ok(Self {
            screen,
            frame: [[b' '; LCD_COLUMNS]; LCD_ROWS],
            backlight: None,
        })
    }

    fn update(
        &mut self,
        lines: &[[u8; LCD_COLUMNS]; LCD_ROWS],
        color: [u8; 3],
    ) -> Result<(), LinuxI2CError> {
        if self.backlight != Some(color) {
            self.screen.change_backlight(color[0], color[1], color[2])?;
            thread::sleep(COMMAND_DELAY);
            self.backlight = Some(color);
        }
        for (row, line) in lines.iter().enumerate() {
            let mut column = 0;
            while column < LCD_COLUMNS {
                if self.frame[row][column] == line[column] {
                    column += 1;
                    continue;
                }
                let start = column;
                while column < LCD_COLUMNS && self.frame[row][column] != line[column] {
                    column += 1;
                }
                self.screen.move_cursor(row, start)?;
                thread::sleep(COMMAND_DELAY);
                for index in start..column {
                    self.screen.write_byte(line[index])?;
                    thread::sleep(CHARACTER_DELAY);
                    self.frame[row][index] = line[index];
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HealthLevel {
    Ok,
    Warning,
    Error,
}

impl HealthLevel {
    fn color(self) -> [u8; 3] {
        match self {
            Self::Ok => [0, 255, 0],
            Self::Warning => [255, 255, 0],
            Self::Error => [255, 0, 0],
        }
    }
}

#[derive(PartialEq, Eq)]
struct HealthStatus {
    level: HealthLevel,
    messages: Vec<String>,
}

impl HealthStatus {
    fn starting() -> Self {
        Self {
            level: HealthLevel::Warning,
            messages: vec!["Starting".into()],
        }
    }

    fn add(&mut self, level: HealthLevel, message: impl Into<String>) {
        if level < self.level {
            return;
        }
        if level > self.level {
            self.messages.clear();
            self.level = level;
        }
        self.messages.push(message.into());
    }
}

fn health_status(
    directory: &Path,
    readings: &[(&str, Option<f64>)],
    controller: Option<&super::web_control::ControllerStatus>,
    network_available: bool,
    starting: bool,
) -> HealthStatus {
    use HealthLevel::{Error, Ok, Warning};
    let mut status = HealthStatus {
        level: Ok,
        messages: vec!["Ok".into()],
    };
    let missing_air: Vec<_> = [
        ("co2", "CO2"),
        ("tvoc", "TVOC"),
        ("temp", "Temp"),
        ("hum", "Humidity"),
    ]
    .iter()
    .filter(|(key, _)| {
        !readings
            .iter()
            .any(|reading| reading.0 == *key && reading.1.is_some())
    })
    .map(|(_, label)| *label)
    .collect();
    if !missing_air.is_empty() {
        status.add(
            Warning,
            format!("Sensor malfunction: {} (S1)", missing_air.join(", ")),
        );
    }
    if ["pm25", "pm10"].iter().any(|key| {
        !readings
            .iter()
            .any(|reading| reading.0 == *key && reading.1.is_some())
    }) {
        status.add(Warning, "Sensor malfunction: PM (S2)");
    }

    let sensors = directory.join("sensors");
    let overflow = super::sensors::overflow_readings_in(&sensors);
    // On communication failure the reader writes OVERFLOW as a fail-safe.
    // Check its error marker after reading the files so those synthetic values
    // are shown as a sensor malfunction, not an actual overflow.
    if sensors.join("overflow_error").exists() {
        status.add(Warning, "Sensor malfunction: overflow (S3)");
    } else {
        for (index, reading) in overflow.iter().enumerate() {
            match reading {
                super::sensors::OverflowReading::Normal => (),
                super::sensors::OverflowReading::Overflow => {
                    status.add(Error, format!("Overflow {} (E{})", index + 1, index + 4))
                }
                _ => status.add(
                    Warning,
                    format!("Sensor malfunction: overflow {} (S3)", index + 1),
                ),
            }
        }
    }

    use super::ph_sensor::LivePhHealth;
    match super::ph_sensor::active_sensor().live_health() {
        LivePhHealth::Normal => (),
        LivePhHealth::Warning => status.add(Warning, "pH outside target (W1)"),
        LivePhHealth::Critical => status.add(Error, "Critical pH (E6)"),
        LivePhHealth::Unavailable => status.add(Warning, "Sensor malfunction: pH (S4)"),
        LivePhHealth::InvalidCalibration => status.add(Error, "Check pH calibration (E7)"),
    }
    if !network_available {
        status.add(Warning, "Network unavailable (W2)");
    }
    if directory.join("emergency_stop").exists() {
        status.add(Error, "Emergency stop (E3)");
    }
    if let Some(controller) = controller {
        if controller.stop_pending {
            status.add(Error, "Relay stop failed (E1)");
        }
        if controller.error.is_some() {
            status.add(Error, "Relay control fault (E2)");
        }
        if controller.outputs.iter().any(|output| {
            output.blocked_reason.as_deref() == Some("Safety input is LOW or unavailable")
        }) {
            status.add(Error, "Safety input active (E9)");
        }
    } else if starting {
        status.add(Warning, "Starting");
    } else {
        status.add(Error, "Controller unavailable (E8)");
    }

    // Let serial discovery complete before reporting missing startup samples.
    // Actual overflows, emergency stops, and controller errors still take priority.
    if starting && status.level == Warning {
        HealthStatus::starting()
    } else {
        status
    }
}

struct StatusTicker {
    status: HealthStatus,
    message: usize,
    offset: usize,
    next_step: Instant,
}

impl StatusTicker {
    fn new(now: Instant) -> Self {
        Self {
            status: HealthStatus::starting(),
            message: 0,
            offset: 0,
            next_step: now + SCROLL_HOLD,
        }
    }

    fn set_status(&mut self, status: HealthStatus, now: Instant) {
        if self.status == status {
            return;
        }
        let message = status.messages.join("; ");
        match status.level {
            HealthLevel::Ok => log::info!("LCD status: {}", message),
            HealthLevel::Warning => log::warn!("LCD status: {}", message),
            HealthLevel::Error => log::error!("LCD status: {}", message),
        }
        self.status = status;
        self.message = 0;
        self.offset = 0;
        self.next_step = now + SCROLL_HOLD;
    }

    fn line(&mut self, now: Instant) -> [u8; LCD_COLUMNS] {
        if now >= self.next_step {
            let end = self.status.messages[self.message]
                .chars()
                .count()
                .saturating_sub(STATUS_COLUMNS);
            if self.offset < end {
                self.offset += 1;
                self.next_step = now
                    + if self.offset == end {
                        SCROLL_HOLD
                    } else {
                        SCROLL_INTERVAL
                    };
            } else {
                self.message = (self.message + 1) % self.status.messages.len();
                self.offset = 0;
                self.next_step = now + SCROLL_HOLD;
            }
        }
        let visible: String = self.status.messages[self.message]
            .chars()
            .skip(self.offset)
            .take(STATUS_COLUMNS)
            .collect();
        lcd_line(&format!("Status: {}", visible))
    }
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
