// Copyright (c) 2020-2021 Caleb Mitchell Smith (PixelCoda)
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
// THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

// Relay 1: Lights + Air
// Relay 2: Drain
// Relay 3: Fill
// Relay 4: Aux Tank Pump

pub mod setup;
pub use ::aog::aog;

// const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
extern crate qwiic_lcd_rs;
extern crate qwiic_relay_rs;

use clap::Parser;

use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;
use simple_logger::SimpleLogger;

use std::io::{stdin, stdout, IsTerminal, Write};

use ::aog::Config;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use error_chain::error_chain;
error_chain! {
    foreign_links {
        Io(std::io::Error);
        Setup(setup::Error);
    }
}

fn main() -> Result<()> {
    let args = ::aog::Args::parse();
    sudo::with_env(&[
        "LIBTORCH",
        "LD_LIBRARY_PATH",
        "PG_DBNAME",
        "PG_USER",
        "PG_PASS",
        "PG_ADDRESS",
    ])
    .map_err(|e| format!("Failed to set environment: {}", e))?;
    setup::install(args.clone())?;

    // Validate permissions on startup
    validate_startup_permissions()?;

    // Check for running instance
    if !aog::instance::handle_instance_check(args.force)
        .map_err(|e| format!("Instance check failed: {}", e))?
    {
        // Another instance is running and force flag not set
        return Ok(());
    }

    // Acquire lock for this instance
    if !aog::instance::acquire_lock()
        .map_err(|e| format!("Failed to acquire instance lock: {}", e))?
    {
        log::error!("Another instance is already running");
        println!("Another AOG instance is already running. Use --force to override.");
        return Ok(());
    }

    let config = Arc::new(Mutex::new(
        Config::load(0).map_err(|e| format!("Failed to load config: {}", e))?,
    ));

    crate::aog::sensors::init();

    // Initialize water level monitoring system
    if let Some(water_config) = config.lock().unwrap().water_level_config.clone() {
        if let Err(e) = crate::aog::water_level::init_water_level_system(water_config) {
            log::warn!("Failed to initialize water level system: {}", e);
        } else {
            log::info!("Water level monitoring system initialized");
        }
    } else {
        log::info!("No water level configuration found, using overflow sensors only");
    }

    // Initialize the LCD
    crate::aog::lcd::init();

    // Initialize the log system
    aog::init_log("/opt/aog/output.log".to_string())
        .map_err(|e| format!("Failed to initialize log: {}", e))?;
    SimpleLogger::new()
        .with_colors(true)
        .with_output_file("/opt/aog/output.log".to_string())
        .init()
        .map_err(|e| format!("Failed to initialize logger: {}", e))?;

    // Turn off all relays
    let qwiic_device = crate::aog::qwiic::QwiicRelayDevice::new(0x25);
    qwiic_device.test();

    // Term now will be true when its time to terminate the software...
    // Ex. CTRL-C, SIGTERM
    // ----------------------------------------------------------------
    let term_now = Arc::new(AtomicBool::new(false));

    // Ask signal_hook to set the term variable to true
    // when the program receives a SIGTERM kill signal
    // ----------------------------------------------------------------
    for sig in TERM_SIGNALS {
        flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now))?;
        flag::register(*sig, Arc::clone(&term_now))?;
    }

    thread::spawn(|| {
        aog::http::init();
    });

    thread::spawn(|| {
        aog::http::init_command_api();
    });

    // Start video thread(s)
    aog::video::init_all();

    // Print banner
    // ----------------------------------------------------------------
    aog::print_stats();

    // Read terminal input separately so blocking stdin cannot delay SIGTERM.
    let (input_tx, input_rx) = mpsc::sync_channel(16);
    thread::spawn(move || {
        let input = stdin();
        let interactive = input.is_terminal();
        loop {
            if interactive {
                print!("> ");
                let _ = stdout().flush();
            }
            let mut line = String::new();
            match input.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if input_tx.send(line).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    log::error!("Failed to read input: {}", error);
                    break;
                }
            }
        }
    });
    process_commands(&input_rx, &term_now, |command| {
        // This process owns the instance lock; execute locally.
        if let Err(error) = aog::command::run(command) {
            log::error!("Command failed: {}", error);
        }
    });

    // Since our loop is basically an infinite loop,
    // that only ends when we receive SIGTERM, if
    // we got here, it's because the loop exited after
    // receiving SIGTERM
    println!("Exiting...");
    aog::web_control::shutdown();

    // Cleanup
    // aog::pump::stop(Arc::clone(&pump_thread));
    // aog::gpio::thread::stop(Arc::clone(&gpio_27_thread));
    // aog::gpio::thread::stop(Arc::clone(&gpio_22_thread));

    // Release instance lock
    if let Err(e) = aog::instance::release_lock() {
        log::warn!("Failed to release instance lock: {}", e);
    }

    Ok(())
}

fn process_commands(
    input: &Receiver<String>,
    terminate: &AtomicBool,
    mut execute: impl FnMut(String),
) {
    let mut input_closed = false;
    while !terminate.load(Ordering::Relaxed) {
        if input_closed {
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        match input.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                let command = line.trim();
                if !command.is_empty() {
                    execute(command.to_owned());
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => input_closed = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_ignores_blank_input_and_executes_commands_locally() {
        let (sender, receiver) = mpsc::channel();
        sender.send(" \r\n".to_owned()).unwrap();
        sender.send(" help\r\n".to_owned()).unwrap();
        let terminate = AtomicBool::new(false);
        let mut commands = Vec::new();
        process_commands(&receiver, &terminate, |command| {
            commands.push(command);
            terminate.store(true, Ordering::Relaxed);
        });
        assert_eq!(commands, ["help"]);
    }

    #[test]
    fn shutdown_works_with_idle_or_closed_stdin() {
        for closed in [false, true] {
            let (sender, receiver) = mpsc::channel();
            let sender = if closed { None } else { Some(sender) };
            let terminate = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&terminate);
            let worker = thread::spawn(move || {
                process_commands(&receiver, &flag, |_| panic!("No command expected"));
            });
            thread::sleep(Duration::from_millis(150));
            assert!(
                !worker.is_finished(),
                "EOF must leave the web services running"
            );
            terminate.store(true, Ordering::Relaxed);
            let start = std::time::Instant::now();
            worker.join().unwrap();
            assert!(start.elapsed() < Duration::from_secs(1));
            drop(sender);
        }
    }
}

/// Validates critical file permissions on startup
fn validate_startup_permissions() -> Result<()> {
    use std::path::Path;

    log::info!("Performing security audit of file permissions...");

    let critical_paths = vec![
        ("/opt/aog", "directory"),
        ("/opt/aog/config.json", "config"),
        ("/opt/aog/output.log", "log"),
        ("/opt/aog/sensors", "directory"),
        ("/opt/aog/crt", "directory"),
        ("/opt/aog/dat", "directory"),
    ];

    let mut issues_found = false;

    for (path, file_type) in critical_paths {
        if Path::new(path).exists() {
            match crate::aog::tools::validate_permissions(path) {
                Ok(valid) => {
                    if !valid {
                        log::error!(
                            "SECURITY ISSUE: {} {} has insecure permissions",
                            file_type,
                            path
                        );
                        security_audit_log(&format!(
                            "Insecure permissions detected on {} {}",
                            file_type, path
                        ));
                        issues_found = true;

                        // Attempt to fix the permissions
                        log::info!("Attempting to fix permissions on {}", path);
                        if let Err(e) = crate::aog::tools::fix_permissions(path) {
                            log::error!("Failed to fix permissions on {}: {}", path, e);
                        } else {
                            security_audit_log(&format!(
                                "Fixed permissions on {} {}",
                                file_type, path
                            ));
                        }
                    } else {
                        log::debug!("{} {} has valid permissions", file_type, path);
                    }
                }
                Err(e) => {
                    log::warn!("Could not validate {} {}: {}", file_type, path, e);
                }
            }
        }
    }

    if issues_found {
        log::warn!("Security issues were found and fixed. Please review the security audit log.");
        security_audit_log("Permission validation completed with issues fixed");
    } else {
        log::info!("All file permissions validated successfully");
        security_audit_log("Permission validation completed successfully");
    }

    Ok(())
}

/// Logs security-related events to a dedicated audit log
fn security_audit_log(message: &str) {
    use chrono::Local;
    use std::fs::OpenOptions;
    use std::io::Write;

    let audit_path = "/opt/aog/security_audit.log";
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_entry = format!("[{}] {}\n", timestamp, message);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_path)
    {
        let _ = file.write_all(log_entry.as_bytes());
        // Set secure permissions on audit log
        let _ = crate::aog::tools::fix_permissions(audit_path);
    }
}
