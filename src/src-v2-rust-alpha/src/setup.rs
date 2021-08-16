use std::env;
use std::process::Command;
use std::io::{Write, stdin, stdout};
use std::path::{Path};
use std::io::prelude::*;
use std::fs::File;
use std::fs;
use std::io;

use crate::aog;

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
            println!("Goodbye...")
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
                aog_config.power_type = "solar";
            } else {
                aog_config.power_type = "grid";
            }


        }

     
  

}