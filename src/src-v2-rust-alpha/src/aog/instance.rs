use std::fs::{self, File};
use std::io::{self, Write, Read};
use std::net::TcpStream;
use std::path::Path;
use std::process;
use std::time::Duration;
use serde::{Deserialize, Serialize};

const PID_FILE: &str = "/opt/aog/aog.pid";
const LOCK_FILE: &str = "/opt/aog/aog.lock";
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 500;

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub pid: u32,
    pub port: u16,
    pub start_time: String,
}

pub fn check_running_instance() -> bool {
    if let Ok(_) = TcpStream::connect("127.0.0.1:9443") {
        return true;
    }
    false
}

pub fn check_port_available(port: u16) -> bool {
    match TcpStream::connect(format!("127.0.0.1:{}", port)) {
        Ok(_) => false,
        Err(_) => true,
    }
}

pub fn write_pid_file() -> io::Result<()> {
    let pid = process::id();
    let info = InstanceInfo {
        pid,
        port: 9443,
        start_time: chrono::Local::now().to_rfc3339(),
    };
    
    let content = serde_json::to_string_pretty(&info)?;
    let mut file = File::create(PID_FILE)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

pub fn read_pid_file() -> io::Result<InstanceInfo> {
    let mut file = File::open(PID_FILE)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let info: InstanceInfo = serde_json::from_str(&content)?;
    Ok(info)
}

pub fn remove_pid_file() -> io::Result<()> {
    if Path::new(PID_FILE).exists() {
        fs::remove_file(PID_FILE)?;
    }
    Ok(())
}

pub fn acquire_lock() -> io::Result<bool> {
    if Path::new(LOCK_FILE).exists() {
        if let Ok(info) = read_pid_file() {
            if !is_process_running(info.pid) {
                log::info!("Removing stale lock file from PID {}", info.pid);
                fs::remove_file(LOCK_FILE).ok();
                remove_pid_file().ok();
            } else {
                return Ok(false);
            }
        }
    }
    
    File::create(LOCK_FILE)?;
    write_pid_file()?;
    Ok(true)
}

pub fn release_lock() -> io::Result<()> {
    remove_pid_file()?;
    if Path::new(LOCK_FILE).exists() {
        fs::remove_file(LOCK_FILE)?;
    }
    Ok(())
}

fn is_process_running(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

pub fn forward_command_with_retry(
    command: &str,
    cert_path: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut last_error = None;
    
    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            log::debug!("Retry attempt {} for command forwarding", attempt);
            std::thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
        }
        
        match forward_command_internal(command, cert_path) {
            Ok(response) => {
                log::debug!("Command forwarded successfully on attempt {}", attempt + 1);
                return Ok(response);
            }
            Err(e) => {
                log::warn!("Command forwarding attempt {} failed: {}", attempt + 1, e);
                last_error = Some(e);
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| "Command forwarding failed after all retries".into()))
}

fn forward_command_internal(
    command: &str,
    cert_path: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let params = [("input_command", command)];
    
    let mut client_builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(true);
    
    if let Some(cert_file) = cert_path {
        if Path::new(cert_file).exists() {
            match std::fs::read(cert_file) {
                Ok(der) => {
                    match reqwest::Certificate::from_der(&der) {
                        Ok(cert) => {
                            client_builder = client_builder.add_root_certificate(cert);
                        }
                        Err(e) => {
                            log::warn!("Failed to parse certificate: {}. Proceeding without it.", e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to read certificate file: {}. Proceeding without it.", e);
                }
            }
        }
    }
    
    let client = client_builder.build()?;
    let response = client
        .post("https://localhost:9443/api/command")
        .form(&params)
        .send()?;
    
    if !response.status().is_success() {
        return Err(format!("Server returned status: {}", response.status()).into());
    }
    
    let body = response.text()?;
    
    // Try to parse JSON response to extract output
    if let Ok(json_response) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(output) = json_response.get("output").and_then(|v| v.as_str()) {
            return Ok(output.to_string());
        }
    }
    
    Ok(body)
}

pub fn handle_instance_check(force: bool) -> Result<bool, Box<dyn std::error::Error>> {
    if force {
        log::info!("Force flag set, bypassing instance check");
        release_lock().ok();
        return Ok(true);
    }
    
    if check_running_instance() {
        if let Ok(info) = read_pid_file() {
            log::info!(
                "AOG instance already running (PID: {}, started: {})",
                info.pid,
                info.start_time
            );
            println!(
                "An AOG instance is already running on port 9443 (PID: {})",
                info.pid
            );
            println!("Use --force to override or stop the existing instance first");
            return Ok(false);
        } else {
            log::warn!("Instance detected on port 9443 but no PID file found");
            println!("An AOG instance appears to be running on port 9443");
            println!("Use --force to override");
            return Ok(false);
        }
    }
    
    Ok(true)
}