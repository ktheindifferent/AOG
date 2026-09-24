//! Run with AOG stopped: sudo target/debug/examples/sensor_diagnostics --seconds 60
//! Reads sensors only; does not initialize relays, pumps, LCD, or HTTP servers.
use aog::aog::{instance, sensors};
use clap::Parser;
use std::time::{Duration, Instant};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value_t = 30)]
    seconds: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if instance::check_running_instance() {
        return Err("Stop AOG before running serial diagnostics".into());
    }
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()?;
    sensors::init_serial_monitoring();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(args.seconds) {
        std::thread::sleep(Duration::from_secs(1));
        println!(
            "t={}s tank1={} tank2={} ph={} co2={} temp={} hum={} pm10={} pm25={} communication_fault={}",
            start.elapsed().as_secs(),
            sensors::get_value("t1_ovf"), sensors::get_value("t2_ovf"),
            sensors::get_value("ph"), sensors::get_value("co2"),
            sensors::get_value("temp"), sensors::get_value("hum"),
            sensors::get_value("pm10"), sensors::get_value("pm25"),
            std::path::Path::new("/opt/aog/sensors/overflow_error").exists(),
        );
    }
    Ok(())
}
