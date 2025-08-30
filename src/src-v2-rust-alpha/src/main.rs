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
pub mod aog;

use aog::error::{AogError, ErrorContext, log_error_with_context};

// const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
extern crate qwiic_lcd_rs;
extern crate qwiic_relay_rs;

use clap::Parser;

use qwiic_relay_rs::*;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;
use simple_logger::SimpleLogger;

use std::io::{stdin,stdout,Write};

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use ::aog::{Config, Session, Sessions};
use std::thread;

use error_chain::error_chain;
error_chain! {
    foreign_links {
        Io(std::io::Error);
        Setup(setup::Error);
    }
}



fn main() -> Result<()> {

    let args = ::aog::Args::parse();
    sudo::with_env(&["LIBTORCH", "LD_LIBRARY_PATH", "PG_DBNAME", "PG_USER", "PG_PASS", "PG_ADDRESS"])
        .map_err(|e| format!("Failed to set environment: {}", e))?;
    setup::install(args.clone())?;
    
    // Validate permissions on startup
    validate_startup_permissions()?;

    let _config = Arc::new(Mutex::new(Config::load(0)
        .map_err(|e| format!("Failed to load config: {}", e))?));

    crate::aog::sensors::init();

    // Initialize the LCD
    crate::aog::lcd::init();

    // Initialize the log system
    aog::init_log("/opt/aog/output.log".to_string())
        .map_err(|e| format!("Failed to initialize log: {}", e))?;
    SimpleLogger::new().with_colors(true).with_output_file("/opt/aog/output.log".to_string()).init()
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


    // A.O.G. Terminal Interface Loop
    // ----------------------------------------------------------------
    while !term_now.load(Ordering::Relaxed) {
        

        let mut s=String::new();
        print!("> ");
        let _=stdout().flush();
        if let Err(e) = stdin().read_line(&mut s) {
            log::error!("Failed to read input: {}", e);
            continue;
        }
        if let Some('\n')=s.chars().next_back() {
            s.pop();
        } 
        if let Some('\r')=s.chars().next_back() {
            s.pop();
        } 

        // Note: Some commands need to be on the main loop for now.
        // ----------------------------------------------------------------



        // TODO
        // If localhost:9443 is available then notify that their is a running background instance.
        // Forward all commands to localhost:8443

        let params = [("input_command", s)];
        // let client = reqwest::Client::new();
        match std::fs::read("/opt/aog/crt/default/aog.local.der") {
            Ok(der) => {
                match reqwest::Certificate::from_der(&der) {
                    Ok(cert) => {
                        match reqwest::blocking::Client::builder()
                            .add_root_certificate(cert)
                            .danger_accept_invalid_certs(true)
                            .build() {
                            Ok(client) => {
                                match client.post(format!("https://localhost:9443").as_str())
                                    .form(&params)
                                    .send() {
                                    Ok(res) => {
                                        match res.text() {
                                            Ok(body) => {
                                                log::debug!("Command response: {}", body);
                                            },
                                            Err(e) => {
                                                let ctx = ErrorContext::new("main", "http_response")
                                                    .with_details(format!("Failed to read response text: {}", e));
                                                let error = AogError::HttpError(e.to_string());
                                                log_error_with_context(&error, &ctx);
                                            }
                                        }
                                    },
                                    Err(e) => log::error!("Failed to send command request: {}", e),
                                }
                            },
                            Err(e) => log::error!("Failed to build HTTP client: {}", e),
                        }
                    },
                    Err(e) => log::error!("Failed to parse certificate: {}", e),
                }
            },
            Err(e) => log::error!("Failed to read certificate file: {}", e),
        }




    }



    

    // Since our loop is basically an infinite loop,
    // that only ends when we receive SIGTERM, if
    // we got here, it's because the loop exited after
    // receiving SIGTERM
    println!("Exiting...");

    // Cleanup
    // aog::pump::stop(Arc::clone(&pump_thread));
    // aog::gpio::thread::stop(Arc::clone(&gpio_27_thread));
    // aog::gpio::thread::stop(Arc::clone(&gpio_22_thread));

    Ok(())
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
                        log::error!("SECURITY ISSUE: {} {} has insecure permissions", file_type, path);
                        security_audit_log(&format!("Insecure permissions detected on {} {}", file_type, path));
                        issues_found = true;
                        
                        // Attempt to fix the permissions
                        log::info!("Attempting to fix permissions on {}", path);
                        if let Err(e) = crate::aog::tools::fix_permissions(path) {
                            log::error!("Failed to fix permissions on {}: {}", path, e);
                        } else {
                            security_audit_log(&format!("Fixed permissions on {} {}", file_type, path));
                        }
                    } else {
                        log::debug!("{} {} has valid permissions", file_type, path);
                    }
                },
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
    use std::fs::OpenOptions;
    use std::io::Write;
    use chrono::Local;
    
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