//! SDS011 query-mode transport. Command acknowledgements (C5) must never be
//! decoded as measurements (C0), even when startup data precedes a reply.
//! Protocol: Nova Fitness, Laser Dust Sensor Control Protocol V1.3.
use sds011::Message;
use serial2::SerialPort;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) struct Sds011 {
    port: SerialPort,
}

impl Sds011 {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        let mut port = SerialPort::open(path, 9600)?;
        port.set_read_timeout(Duration::from_millis(200))?;
        port.set_write_timeout(Duration::from_secs(2))?;
        port.discard_input_buffer()?;
        let sensor = Self { port };
        sensor.setting(0x02, 1)?; // Query reporting, not unsolicited packets.
                                  // Work periods are MINUTES. Zero keeps measurements continuous while
                                  // the host queries every ten seconds; ten means a ten-minute cycle.
        sensor.setting(0x08, 0)?;
        sensor.setting(0x06, 1)?; // Wake a sensor previously left asleep.
        Ok(sensor)
    }

    fn send(&self, command: u8, mode: u8, value: u8) -> io::Result<()> {
        let mut packet = [0u8; 19];
        packet[0] = 0xaa;
        packet[1] = 0xb4;
        packet[2] = command;
        packet[3] = mode;
        packet[4] = value;
        packet[15] = 0xff;
        packet[16] = 0xff;
        packet[17] = checksum(&packet[2..17]);
        packet[18] = 0xab;
        self.port.write_all(&packet)
    }

    fn setting(&self, command: u8, value: u8) -> io::Result<()> {
        self.send(command, 0, 0)?;
        let current =
            self.reply(|frame| frame[1] == 0xc5 && frame[2] == command && frame[3] == 0)?;
        // Reporting mode and work period persist across power cycles. Avoid
        // repeatedly writing those settings when they already match.
        if current[4] != value {
            self.send(command, 1, value)?;
            self.reply(|frame| {
                frame[1] == 0xc5 && frame[2] == command && frame[3] == 1 && frame[4] == value
            })?;
        }
        Ok(())
    }

    pub(super) fn query(&self) -> io::Result<Message> {
        self.send(0x04, 0, 0)?;
        let frame = self.reply(|frame| frame[1] == 0xc0)?;
        let pm25 = f32::from(u16::from_le_bytes([frame[2], frame[3]])) / 10.0;
        let pm10 = f32::from(u16::from_le_bytes([frame[4], frame[5]])) / 10.0;
        if pm25 > 999.9 || pm10 > 999.9 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Particulate measurement is out of range",
            ));
        }
        Ok(Message {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
            pm25,
            pm10,
        })
    }

    fn reply(&self, expected: impl Fn(&[u8; 10]) -> bool) -> io::Result<[u8; 10]> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut frame = [0u8; 10];
        let mut used = 0;
        while Instant::now() < deadline {
            match self.port.read(&mut frame[used..used + 1]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Particulate sensor disconnected",
                    ))
                }
                Ok(_) => used += 1,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::Interrupted
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                    ) =>
                {
                    continue
                }
                Err(error) => return Err(error),
            }
            if used == frame.len() {
                if frame[0] == 0xaa
                    && frame[9] == 0xab
                    && frame[8] == checksum(&frame[2..8])
                    && expected(&frame)
                {
                    return Ok(frame);
                }
                // Sliding resynchronization tolerates incomplete, noisy, or
                // unrelated frames without accepting a mismatched reply.
                frame.copy_within(1.., 0);
                used -= 1;
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "No matching particulate sensor reply",
        ))
    }
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}
