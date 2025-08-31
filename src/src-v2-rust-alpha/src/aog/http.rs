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



use std::path::{Path};


use rouille::Response;
use rouille::post_input;
use rouille::session;
use rouille::try_or_400;

use std::sync::Mutex;



use std::sync::Arc;
use std::collections::HashMap;
use std::time::{SystemTime, Duration};






use serde::{Serialize, Deserialize};


use crate::aog;
use crate::Config;



// Add Debug Flag and use ./www/ instead of installed dir

pub fn init(){


    let config = Arc::new(Mutex::new(match Config::load(0) {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("Failed to load config: {}", e);
            return;
        }
    }));
    
    let cert = match std::fs::read("/opt/aog/crt/default/aog.local.cert") {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to read certificate: {}", e);
            return;
        }
    };
    
    let pkey = match std::fs::read("/opt/aog/crt/default/aog.local.key") {
        Ok(k) => k,
        Err(e) => {
            log::error!("Failed to read private key: {}", e);
            return;
        }
    };
    
    // Get binding configuration from config
    let bind_config = config.lock().unwrap();
    let bind_address = bind_config.https_bind_address.clone().unwrap_or_else(|| "127.0.0.1".to_string());
    let bind_port = bind_config.https_bind_port.unwrap_or(8443);
    let bind_addr = format!("{}:{}", bind_address, bind_port);
    drop(bind_config);
    
    log::info!("Starting HTTPS server on {}", bind_addr);
    
    rouille::Server::new_ssl(bind_addr, move |request| {
        {
            session::session(request, "SID", 3600, |session| {
                let session_id: &str = session.id();
                let mut session_authenticated = false;
                let mut sessions :Vec<crate::Session> = Vec::new();
    
    
                if Path::new("/opt/aog/dat/sessions.bin").exists() {
                    sessions = crate::Sessions::load(0).unwrap().sessions;
                }
    
                for session in &sessions{
                    if session.id.contains(session_id){
                        session_authenticated = true;
                    } 
                }
    
 
    
                let edit_aog_config = &mut *config.lock().unwrap();
    
    
    
                if request.url() == "/authenticate"{
                
                    let input = try_or_400!(post_input!(request, {
                        input_username: String,
                        input_password: String,
                    }));
                    // Use secure password verification instead of plain text comparison
                    let password_valid = match crate::aog::auth::verify_password(&input.input_password, &edit_aog_config.encrypted_password) {
                        Ok(valid) => valid,
                        Err(e) => {
                            log::error!("Password verification error: {}", e);
                            false
                        }
                    };
                    
                    if input.input_username == *"admin" && password_valid {
                                            let session = crate::Session {
                                                id: session_id.to_string(),
                                                delta: 0
                                            };
                                            sessions.push(session);
                    
                                            let _session_save_file = crate::Sessions {
                                                sessions: sessions.clone()
                                            };
                    
                                            // save_file("/opt/aog/dat/sessions.bin", 0, &session_save_file).unwrap();
                                            
                                        }
                  
                    let response = Response::redirect_302("/index.html");
                    return response;
    
    
                }
    

                
    
                if request.url().contains("/api/dat/"){


                    if let Some(request) = request.remove_prefix("/api/dat/") {
                        return rouille::match_assets(&request, "/opt/aog/dat").with_additional_header("Access-Control-Allow-Origin", "*").with_no_cache();
                    } else {
                        return Response::text("err".to_string())
                            .with_additional_header("Access-Control-Allow-Origin", "*");
                    }


                }
    
                // New API endpoint for overflow alerts
                if request.url() == "/api/alerts/overflow" {
                    #[derive(Serialize, Deserialize, Debug, Clone)]
                    struct OverflowAlert {
                        tank1_overflow: bool,
                        tank2_overflow: bool,
                        sensor_error: bool,
                        error_message: String,
                        timestamp: u64,
                        critical: bool
                    }
                    
                    let t1_status = crate::aog::sensors::get_value("t1_ovf");
                    let t2_status = crate::aog::sensors::get_value("t2_ovf");
                    let sensor_error = std::path::Path::new("/opt/aog/sensors/overflow_error").exists();
                    
                    let error_message = if sensor_error {
                        match std::fs::read_to_string("/opt/aog/sensors/overflow_error") {
                            Ok(msg) => msg,
                            Err(_) => "Sensor communication failure".to_string()
                        }
                    } else {
                        "".to_string()
                    };
                    
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    
                    let tank1_overflow = t1_status.contains("OVERFLOW");
                    let tank2_overflow = t2_status.contains("OVERFLOW");
                    let critical = tank1_overflow || tank2_overflow || sensor_error;
                    
                    let response = Response::json(&OverflowAlert {
                        tank1_overflow,
                        tank2_overflow,
                        sensor_error,
                        error_message,
                        timestamp,
                        critical
                    });
                    
                    return response;
                }
                
                // Error monitoring dashboard endpoint
                if request.url() == "/api/errors" {
                    let dashboard = crate::aog::error_monitor::GLOBAL_ERROR_MONITOR.get_dashboard();
                    let response = Response::json(&dashboard);
                    return response.with_additional_header("Access-Control-Allow-Origin", "*");
                }
                
                if request.url() == "/api/stats"{
                    #[derive(Serialize, Deserialize, Debug, Clone)]
                    struct WebApiStats {
                        pm25: String,
                        pm10: String,
                        co2: String,
                        tvoc: String,
                        temp: String,
                        hum: String,
                        t1_ovf: String,
                        t2_ovf: String,
                        overflow_error: bool
                    }
                   
                    let overflow_error = std::path::Path::new("/opt/aog/sensors/overflow_error").exists();
                    let response = Response::json(&WebApiStats { 
                        co2: crate::aog::sensors::get_value("co2"), 
                        tvoc: crate::aog::sensors::get_value("tvoc"), 
                        temp: crate::aog::sensors::get_value("temp"), 
                        hum: crate::aog::sensors::get_value("hum"), 
                        pm25: crate::aog::sensors::get_value("pm25"), 
                        pm10: crate::aog::sensors::get_value("pm10"),
                        t1_ovf: crate::aog::sensors::get_value("t1_ovf"),
                        t2_ovf: crate::aog::sensors::get_value("t2_ovf"),
                        overflow_error: overflow_error
                    });
                    return response;
                }
    
    
                // catchall regardless of auth status
                if request.url() == "/login.html" || request.url().contains(".css") || request.url().contains(".js") || request.url().contains(".png") || request.url().contains(".jpg") || request.url().contains(".tff") || request.url().contains(".woff") || request.url().contains(".woff2") {
                    let response = rouille::match_assets(request, "/opt/aog/www/");
                    if response.is_success() {
                        return response.with_additional_header("Access-Control-Allow-Origin", "*").with_no_cache();
                    } else {
                        return Response::html("404 error").with_status_code(404).with_additional_header("Access-Control-Allow-Origin", "*");
                    }
                }
            
            
    
    
                if session_authenticated{
                    let response = rouille::match_assets(request, "/opt/aog/www/");
                    if response.is_success() {
                        response.with_additional_header("Access-Control-Allow-Origin", "*").with_no_cache()
                    } else {
                        Response::html("404 error").with_status_code(404).with_additional_header("Access-Control-Allow-Origin", "*")
                    }
                } else {
                    //unathuenticated
                    
                    Response::redirect_302("/login.html")
                }
    
          
    
      
            })
    
    
        }
    }, cert, pkey)
    .map_err(|e| log::error!("Failed to start HTTPS server: {}", e))
    .ok()
    .map(|server| server.run());
    
}


// Rate limiting for API endpoints
struct RateLimiter {
    requests: HashMap<String, Vec<SystemTime>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    fn new(max_requests: usize, window_seconds: u64) -> Self {
        RateLimiter {
            requests: HashMap::new(),
            max_requests,
            window: Duration::from_secs(window_seconds),
        }
    }
    
    fn check_rate_limit(&mut self, client_id: &str) -> bool {
        let now = SystemTime::now();
        let window_start = now - self.window;
        
        // Get or create request history for this client
        let requests = self.requests.entry(client_id.to_string()).or_insert_with(Vec::new);
        
        // Remove old requests outside the window
        requests.retain(|&req_time| req_time > window_start);
        
        // Check if limit exceeded
        if requests.len() >= self.max_requests {
            return false;
        }
        
        // Add current request
        requests.push(now);
        true
    }
}

// Command API with enhanced security - localhost only + token authentication
pub fn init_command_api(){

    let config = Arc::new(Mutex::new(match Config::load(0) {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("Failed to load config: {}", e);
            return;
        }
    }));
    
    let cert = match std::fs::read("/opt/aog/crt/default/aog.local.cert") {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to read certificate: {}", e);
            return;
        }
    };
    
    let pkey = match std::fs::read("/opt/aog/crt/default/aog.local.key") {
        Ok(k) => k,
        Err(e) => {
            log::error!("Failed to read private key: {}", e);
            return;
        }
    };
    
    // Get binding configuration from config - FORCE localhost only for security
    let bind_config = config.lock().unwrap();
    // Always bind to localhost regardless of config for security
    let bind_address = "127.0.0.1".to_string();
    let bind_port = bind_config.command_api_bind_port.unwrap_or(9443);
    let bind_addr = format!("{}:{}", bind_address, bind_port);
    let api_token = bind_config.command_api_token.clone();
    drop(bind_config);
    
    // Initialize rate limiter: 10 requests per 60 seconds
    let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(10, 60)));
    
    log::info!("Starting Command API server on {} (localhost-only)", bind_addr);
    
    rouille::Server::new_ssl(bind_addr, move |request| {
        {
            // IP filtering - reject non-localhost connections
            let remote_addr = request.remote_addr();
            let is_localhost = match remote_addr {
                std::net::SocketAddr::V4(addr) => {
                    let ip = addr.ip();
                    ip.is_loopback() || ip.to_string() == "127.0.0.1"
                },
                std::net::SocketAddr::V6(addr) => {
                    let ip = addr.ip();
                    ip.is_loopback() || ip.to_string() == "::1"
                }
            };
            
            if !is_localhost {
                log::warn!("Rejected non-localhost connection from: {}", remote_addr);
                return Response::text("Forbidden: localhost connections only")
                    .with_status_code(403);
            }
            
            // Token authentication check
            let auth_header = request.header("Authorization");
            let token_valid = if let Some(ref expected_token) = api_token {
                match auth_header {
                    Some(header_value) => {
                        // Support both "Bearer <token>" and plain token formats
                        let token = if header_value.starts_with("Bearer ") {
                            &header_value[7..]
                        } else {
                            header_value
                        };
                        token == expected_token
                    },
                    None => false
                }
            } else {
                // If no token configured, authentication is not required (backward compatibility)
                true
            };
            
            if !token_valid {
                log::warn!("Invalid or missing API token from: {}", remote_addr);
                return Response::text("Unauthorized: Invalid API token")
                    .with_status_code(401);
            }
            
            // Rate limiting check
            let client_id = remote_addr.to_string();
            let mut limiter = rate_limiter.lock().unwrap();
            if !limiter.check_rate_limit(&client_id) {
                log::warn!("Rate limit exceeded for client: {}", client_id);
                return Response::text("Too Many Requests")
                    .with_status_code(429)
                    .with_additional_header("Retry-After", "60");
            }
            drop(limiter);
       
            #[derive(Serialize, Deserialize, Debug, Clone)]
            struct CommandStatus {
                status: String
            }
            
            let input = try_or_400!(post_input!(request, {
                input_command: String,
            }));
            
            // Validate and sanitize input command
            let command = input.input_command.trim();
            
            // Define allowed commands whitelist
            let allowed_commands = vec![
                "help", "cls", "clear", "gpio status", "stdout", "test",
                "pump status", "pump fill", "pump drain", "pump stop",
                "relay status"
            ];
            
            // Check if command is in whitelist or is a safe gpio command
            let is_safe = allowed_commands.contains(&command) ||
                         (command.starts_with("gpio on ") && command.split_whitespace().count() == 3) ||
                         (command.starts_with("gpio off ") && command.split_whitespace().count() == 3) ||
                         (command.starts_with("relay on ") && command.split_whitespace().count() == 3) ||
                         (command.starts_with("relay off ") && command.split_whitespace().count() == 3);
            
            if !is_safe {
                log::warn!("Blocked potentially unsafe command: {}", command);
                let response = Response::json(&CommandStatus { status: "blocked: unauthorized command".to_string() });
                return response;
            }
            
            // Additional validation for gpio/relay commands
            if command.starts_with("gpio ") || command.starts_with("relay ") {
                let parts: Vec<&str> = command.split_whitespace().collect();
                if parts.len() >= 3 {
                    // Validate pin/relay number is numeric
                    if parts[2].parse::<u8>().is_err() {
                        log::warn!("Invalid pin/relay number in command: {}", command);
                        let response = Response::json(&CommandStatus { status: "error: invalid pin/relay number".to_string() });
                        return response;
                    }
                }
            }
            
            if input.input_command == *"admin" {
                
            }


            let _ = aog::command::run(input.input_command);

            // let arduino_response = crate::aog::sensors::get_arduino_raw();
            let response = Response::json(&CommandStatus { status: "success".to_string() });
            return response;


        }
    }, cert, pkey)
    .map_err(|e| log::error!("Failed to start HTTPS server: {}", e))
    .ok()
    .map(|server| server.run());
    
}
