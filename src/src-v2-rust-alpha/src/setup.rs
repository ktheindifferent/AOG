use std::env;
use std::process::Command;
use std::io::{Write, stdin, stdout};
use std::path::{Path};
use std::io::prelude::*;
use std::fs::File;
use std::fs;
use std::io;

use crate::aog;

extern crate savefile;
use savefile::prelude::*;

pub fn install() {


        let mut abort_install = true;

        let mut s=String::new();
        print!("Do you want to install A.O.G (Y/n): ");
        let _=stdout().flush();
        stdin().read_line(&mut s).expect("Did not enter a correct string");
        if let Some('\n')=s.chars().next_back() {
            s.pop();
        }
        if let Some('\r')=s.chars().next_back() {
            s.pop();
        }
        if s.contains("Y") || s.contains("y") {
            abort_install = false;
        } else {
            println!("Skipping Setup...")
        }

        if !abort_install {


            let mut aog_config = aog::Config::default();

            // Automatic updates
            let mut s=String::new();
            print!("Enable automatic updates? (Y/n): ");
            let _=stdout().flush();
            stdin().read_line(&mut s).expect("Did not enter a correct string");
            if let Some('\n')=s.chars().next_back() {
                s.pop();
            }
            if let Some('\r')=s.chars().next_back() {
                s.pop();
            }
            if s.contains("Y") || s.contains("y") {
                aog_config.enable_automatic_updates = true;
            }



            let mut s=String::new();
            print!("Does the unit tie into an HVAC system? (Y/n): ");
            let _=stdout().flush();
            stdin().read_line(&mut s).expect("Did not enter a correct string");
            if let Some('\n')=s.chars().next_back() {
                s.pop();
            }
            if let Some('\r')=s.chars().next_back() {
                s.pop();
            }
            if s.contains("Y") || s.contains("y") {
                aog_config.is_hvac_kit_installed = true;
            }

            let mut s=String::new();
            print!("Does the unit have a sensor kit? (Y/n): ");
            let _=stdout().flush();
            stdin().read_line(&mut s).expect("Did not enter a correct string");
            if let Some('\n')=s.chars().next_back() {
                s.pop();
            }
            if let Some('\r')=s.chars().next_back() {
                s.pop();
            }
            if s.contains("Y") || s.contains("y") {
                aog_config.is_sensor_kit_installed = true;

                // TODO collect sensor pinout config from user and flash arduino

            }

            let mut s=String::new();
            print!("Is the unit solar powered? (Y/n): ");
            let _=stdout().flush();
            stdin().read_line(&mut s).expect("Did not enter a correct string");
            if let Some('\n')=s.chars().next_back() {
                s.pop();
            }
            if let Some('\r')=s.chars().next_back() {
                s.pop();
            }
            if s.contains("Y") || s.contains("y") {
                aog_config.power_type = "solar".to_string();
            } else {
                aog_config.power_type = "grid".to_string();
            }


            let mut sudo_password = String::new();
            print!("Enter 'sudo' password: ");
            let _=stdout().flush();
            stdin().read_line(&mut sudo_password).expect("Did not enter a correct string");
            println!();
        
        
            let step1 = Command::new("sh")
            .arg("-c")
            .arg(format!("echo \"{}\" | sudo -S mkdir /opt/aog", sudo_password))
            .output()
            .expect("failed to execute process");
            if step1.status.success() {
                println!("");
            } else {
                let er = String::from_utf8_lossy(&step1.stderr);
                println!("{}", er);
            }
        
            let step2 = Command::new("sh")
            .arg("-c")
            .arg(format!("echo \"{}\" | sudo -S chmod -R 777 /opt/aog", sudo_password))
            .output()
            .expect("failed to execute process");
            if step2.status.success() {
                println!("");
            } else {
                let er = String::from_utf8_lossy(&step2.stderr);
                println!("{}", er);
            }
        
            let step3 = Command::new("sh")
            .arg("-c")
            .arg(format!("echo \"{}\" | sudo -S chown 1000 -R /opt/aog", sudo_password))
            .output()
            .expect("failed to execute process");
            if step3.status.success() {
                println!("");
            } else {
                let er = String::from_utf8_lossy(&step3.stderr);
                println!("{}", er);
            }
        
            Command::new("sh")
            .arg("-c")
            .arg("mkdir /opt/aog/bak")
            .output()
            .expect("failed to execute process");
        
          
        
            
            save_file("/opt/aog/config.bin", 0, &aog_config).unwrap();



            aog::cls();


        }

     
  

}

pub fn update(){

    let mut s=String::new();
    print!("Do you want to update A.O.G (Y/n): ");
    let _=stdout().flush();
    stdin().read_line(&mut s).expect("Did not enter a correct string");
    if let Some('\n')=s.chars().next_back() {
        s.pop();
    }
    if let Some('\r')=s.chars().next_back() {
        s.pop();
    }
    if s.contains("Y") || s.contains("y") {
        uninstall();
        install();
    } else {
        println!("Skipping Update...")
    }

}

pub fn uninstall(){
    Command::new("sh")
    .arg("-c")
    .arg("rm -rf /opt/aog")
    .output()
    .expect("failed to execute process");
}