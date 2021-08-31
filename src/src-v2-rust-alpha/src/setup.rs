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


use std::process::Command;
use std::io::{Write, stdin, stdout};
use std::path::{Path};
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
        if s.contains('Y') || s.contains('y') {
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
            if s.contains('Y') || s.contains('y') {
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
            if s.contains('Y') || s.contains('y') {
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
            if s.contains('Y') || s.contains('y') {
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
            if s.contains('Y') || s.contains('y') {
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
                println!();
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
                println!();
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
                println!();
            } else {
                let er = String::from_utf8_lossy(&step3.stderr);
                println!("{}", er);
            }
        
            Command::new("sh")
            .arg("-c")
            .arg("mkdir /opt/aog/bak")
            .output()
            .expect("failed to execute process");

            Command::new("sh")
            .arg("-c")
            .arg("mkdir /opt/aog/crt")
            .output()
            .expect("failed to execute process");
        
            Command::new("sh")
            .arg("-c")
            .arg("mkdir /opt/aog/crt/default")
            .output()
            .expect("failed to execute process");
        
            Command::new("sh")
            .arg("-c")
            .arg("mkdir /opt/aog/dat")
            .output()
            .expect("failed to execute process");
        
            let openssh = Command::new("/bin/bash")
            .arg("-c")
            .arg(" openssl req -x509 -out /opt/aog/crt/default/aog.local.cert -keyout /opt/aog/crt/default/aog.local.key \
            -newkey rsa:2048 -nodes -sha256 \
            -subj '/CN=localhost' -extensions EXT -config <( \
             printf \"[dn]\nCN=localhost\n[req]\ndistinguished_name = dn\n[EXT]\nsubjectAltName=DNS:localhost\nkeyUsage=digitalSignature\nextendedKeyUsage=serverAuth\")")
            .output()
            .expect("failed to execute process");
            if openssh.status.success() {
                println!();
            } else {
                let er = String::from_utf8_lossy(&openssh.stderr);
                println!("{}", er);
            }
        
            let openssh_der = Command::new("/bin/bash")
            .arg("-c")
            .arg("openssl x509 -outform der -in /opt/aog/crt/default/aog.local.cert -out /opt/aog/crt/default/aog.local.der")
            .output()
            .expect("failed to execute process");
            if openssh_der.status.success() {
                println!();
            } else {
                let er = String::from_utf8_lossy(&openssh_der.stderr);
                println!("{}", er);
            }
        
        
        
          
            let www_build = rebuild_www();

            if www_build.is_ok() {
                Command::new("sh")
                .arg("-c")
                .arg("rm -rf /opt/aog/www.zip")
                .output()
                .expect("failed to execute process");    
            }



            
            save_file("/opt/aog/config.bin", 0, &aog_config).unwrap();



            aog::cls();


        }

     
  

}

fn rebuild_www() -> std::io::Result<()> {

    let data = include_bytes!("www.zip");

    let mut pos = 0;
    let mut buffer = File::create("/opt/aog/www.zip")?;

    while pos < data.len() {
        let bytes_written = buffer.write(&data[pos..])?;
        pos += bytes_written;
    }

    extract_zip("/opt/aog/www.zip");
    Ok(())
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
    if s.contains('Y') || s.contains('y') {
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


fn extract_zip(zip_path: &str) -> i32 {

    let fname = std::path::Path::new(zip_path);
    let file = fs::File::open(&fname).unwrap();

    let mut archive = zip::ZipArchive::new(file).unwrap();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath_end = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        let out_mend = "/opt/aog/".to_owned() + outpath_end.to_str().unwrap();

        let outpath = Path::new(&(out_mend));

        {
            let comment = file.comment();
            if !comment.is_empty() {
                // println!("File {} comment: {}", i, comment);
            }
        }

        if (&*file.name()).ends_with('/') {
            // println!("File {} extracted to \"{}\"", i, outpath.display());
            fs::create_dir_all(&outpath).unwrap();
        } else {
            // println!(
            //     "File {} extracted to \"{}\" ({} bytes)",
            //     i,
            //     outpath.display(),
            //     file.size()
            // );
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p).unwrap();
                }
            }
            let mut outfile = fs::File::create(&outpath).unwrap();
            io::copy(&mut file, &mut outfile).unwrap();
        }

        // Get and Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode)).unwrap();
            }
        }
    }
    0
}