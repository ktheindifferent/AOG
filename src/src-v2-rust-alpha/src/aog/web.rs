use std::io::Error;
use std::io::{Write, stdin, stdout};
use std::path::{Path};

use rouille::Request;
use rouille::Response;
use rouille::post_input;
use rouille::session;
use rouille::try_or_400;

use std::sync::Mutex;

use std::thread;

use std::sync::Arc;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};


use serde::{Serialize, Deserialize};
use shuteye::sleep;

use std::sync::atomic::{AtomicI64, AtomicBool, Ordering};
use std::time::Duration;



extern crate savefile;
use savefile::prelude::*;

pub fn init(){


    let mut config = Arc::new(Mutex::new(crate::aog::load_config().unwrap()));

    

    let mut cert = std::fs::read("/opt/aog/crt/default/aog.local.cert").unwrap();
    let mut pkey = std::fs::read("/opt/aog/crt/default/aog.local.key").unwrap();
    
    rouille::Server::new_ssl("0.0.0.0:8043", move |request| {
        {
            session::session(request, "SID", 3600, |session| {
    
    
                let session_id: &str = session.id();
                let mut session_authenticated = false;
                let mut sessions :Vec<crate::aog::Session> = Vec::new();
    
    
                if Path::new("/opt/aog/dat/sessions.bin").exists() {
                    sessions = crate::aog::load_sessions(Arc::clone(&config)).unwrap().sessions;
                }
    
                for session in &sessions{
                    if session.id.contains(session_id){
                        session_authenticated = true;
                    } 
                }
    
                // catchall regardless of auth status
                if request.url() == "/login.html" || request.url().contains(".css") || request.url().contains(".js") || request.url().contains(".png") || request.url().contains(".jpg") || request.url().contains(".tff") || request.url().contains(".woff") || request.url().contains(".woff2") {
                    let response = rouille::match_assets(&request, "./www/");
                    if response.is_success() {
                        return response.with_additional_header("Access-Control-Allow-Origin", "*").with_no_cache();
                    } else {
                        return Response::html("404 error").with_status_code(404).with_additional_header("Access-Control-Allow-Origin", "*");
                    }
                }
    
    
    
                let mut edit_aog_config = &mut *config.lock().unwrap();
    
    
    
                if request.url() == "/authenticate"{
                
                    let input = try_or_400!(post_input!(request, {
                        inputUsername: String,
                        inputPassword: String,
                    }));
                    if input.inputUsername == "admin".to_string(){
                        if input.inputPassword.clone() == edit_aog_config.encrypted_password{
                            let session = crate::aog::Session {
                                id: session_id.to_string(),
                                delta: 0
                            };
                            sessions.push(session);
    
                            let session_save_file = crate::aog::Sessions {
                                sessions: sessions.clone()
                            };
    
                            save_file("/opt/aog/dat/sessions.bin", 0, &session_save_file).unwrap();
                            
                        }
                    }
                  
                    let response = Response::redirect_302("/index.html");
                    return response;
    
    
                }
    
    
    
                if request.url() == "/api/stats"{
                    #[derive(Serialize, Deserialize, Savefile, Debug, Clone)]
                    struct WebApiStats {
                        pm25: String,
                        pm10: String,
                        co2: String,
                        tvoc: String,
                        temp: String,
                        hum: String
                    }
                    let arduino_response = crate::aog::sensors::get_arduino_raw();
                    let response = Response::json(&WebApiStats { co2: crate::aog::sensors::get_co2(arduino_response.clone()), tvoc: crate::aog::sensors::get_tvoc(arduino_response.clone()), temp: crate::aog::sensors::get_temperature(arduino_response.clone()), hum: crate::aog::sensors::get_humidity(arduino_response.clone()), pm25: crate::aog::sensors::get_pm25(), pm10: crate::aog::sensors::get_pm10() });
                    return response;
                }
    
    
         
    
    
                if session_authenticated{
                    let response = rouille::match_assets(&request, "./www/");
                    if response.is_success() {
                        return response.with_additional_header("Access-Control-Allow-Origin", "*").with_no_cache();
                    } else {
                        return Response::html("404 error").with_status_code(404).with_additional_header("Access-Control-Allow-Origin", "*");
                    }
                } else {
                    //unathuenticated
                    let response = Response::redirect_302("/login.html");
                    return response;
                }
    
          
    
      
            })
    
    
        }
    }, cert, pkey).unwrap().run();
    
}
