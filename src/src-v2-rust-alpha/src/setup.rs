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
use std::io::{Write};
use std::path::{Path};
use std::fs::File;
use std::fs;
use std::io;




// use std::io::Write;
use error_chain::error_chain;

error_chain! {
    foreign_links {
        Io(std::io::Error);
        // Hound(hound::Error);
        ToolKitError(crate::aog::tools::Error);
    }
}

pub fn install(_args: aog::Args) -> Result<()> {

    match crate::aog::tools::mkdir("/opt"){
        Ok(_) => {},
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to create /opt directory").into()),
    }

    match crate::aog::tools::mkdir("/opt/aog"){
        Ok(_) => {},
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to create /opt/aog directory").into()),
    }

    match crate::aog::tools::fix_permissions("/opt/aog"){
        Ok(_) => {},
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to chmod /opt/aog").into()),
    }

    match crate::aog::tools::mkdir("/opt/aog/bak"){
        Ok(_) => {},
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to chmod /opt/aog/bak").into()),
    }

    match crate::aog::tools::mkdir("/opt/aog/sensors"){
        Ok(_) => {},
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to mkdir /opt/aog/sensors").into()),
    }

    match crate::aog::tools::mkdir("/opt/aog/crt"){
        Ok(_) => {},
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to mkdir /opt/aog/crt").into()),
    }

    match crate::aog::tools::mkdir("/opt/aog/crt/default"){
        Ok(_) => {},
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to mkdir /opt/aog/crt/default").into()),
    }
    
    match crate::aog::tools::mkdir("/opt/aog/dat"){
        Ok(_) => {},
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to mkdir /opt/aog/dat").into()),
    }
    

    let openssh = Command::new("/bin/bash")
    .arg("-c")
    .arg(" openssl req -x509 -out /opt/aog/crt/default/aog.local.cert -keyout /opt/aog/crt/default/aog.local.key \
    -newkey rsa:2048 -nodes -sha256 \
    -subj '/CN=localhost' -extensions EXT -config <( \
        printf \"[dn]\nCN=localhost\n[req]\ndistinguished_name = dn\n[EXT]\nsubjectAltName=DNS:localhost\nkeyUsage=digitalSignature\nextendedKeyUsage=serverAuth\")")
    .output()
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to generate SSL certificate: {}", e)))?;
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
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to convert certificate to DER: {}", e)))?;
    if openssh_der.status.success() {
        println!();
    } else {
        let er = String::from_utf8_lossy(&openssh_der.stderr);
        println!("{}", er);
    }

    
    let www_build = rebuild_www();

    if www_build.is_ok() {
        if let Err(e) = Command::new("sh")
        .arg("-c")
        .arg("rm -rf /opt/aog/www.zip")
        .output() {
            log::warn!("Failed to remove www.zip: {}", e);
        }    
    }

    Ok(())
}

fn rebuild_www() -> std::io::Result<()> {

    let data = include_bytes!("www.zip");

    let mut pos = 0;
    let mut buffer = File::create("/opt/aog/www.zip")?;

    while pos < data.len() {
        let bytes_written = buffer.write(&data[pos..])?;
        pos += bytes_written;
    }

    extract_zip("/opt/aog/www.zip")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to extract www.zip: {:?}", e)))?;
    Ok(())
}



pub fn update(){
    // uninstall();
    // install();
}

pub fn uninstall(){
    if let Err(e) = Command::new("sh")
    .arg("-c")
    .arg("rm -rf /opt/aog")
    .output() {
        log::error!("Failed to remove /opt/aog directory: {}", e);
    }
}


fn extract_zip(zip_path: &str) -> Result<i32> {

    let fname = std::path::Path::new(zip_path);
    let file = fs::File::open(&fname)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to open zip file: {}", e)))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to read zip archive: {}", e)))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to read file from archive: {}", e)))?;
        let outpath_end = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        let out_mend = "/opt/aog/".to_owned() + outpath_end.to_str()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Invalid path in archive"))?;

        let outpath = Path::new(&(out_mend));

        {
            let comment = file.comment();
            if !comment.is_empty() {
                // println!("File {} comment: {}", i, comment);
            }
        }

        if (&*file.name()).ends_with('/') {
            // println!("File {} extracted to \"{}\"", i, outpath.display());
            fs::create_dir_all(&outpath)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create directory: {}", e)))?;
        } else {
            // println!(
            //     "File {} extracted to \"{}\" ({} bytes)",
            //     i,
            //     outpath.display(),
            //     file.size()
            // );
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create parent directory: {}", e)))?;
                }
            }
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create output file: {}", e)))?;
            io::copy(&mut file, &mut outfile)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to copy file content: {}", e)))?;
        }

        // Get and Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            if let Some(mode) = file.unix_mode() {
                if let Err(e) = fs::set_permissions(&outpath, fs::Permissions::from_mode(mode)) {
                    log::warn!("Failed to set permissions on {}: {}", outpath.display(), e);
                }
            }
        }
    }
    Ok(0)
}



pub fn install_service(args: aog::Args) -> Result<()> {

    // Linux
    #[cfg(all(target_os = "linux"))] {
        update_linux_service_file(args.clone());
        match crate::aog::tools::systemctl_reload(){
            Ok(_) => {},
            Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to reload systemctl").into()),
        }
        match crate::aog::tools::systemctl_enable("aog.service"){
            Ok(_) => {},
            Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to enable aog as a service").into()),
        }
        match crate::aog::tools::systemctl_stop("aog.service"){
            Ok(_) => {},
            Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to stop aog as a service").into()),
        }
        // Copy Files
        match std::env::current_exe() {
            Ok(exe_path) => {
                let current_exe_path = format!("{}", exe_path.display());
                match crate::aog::tools::cp(current_exe_path.as_str(), "/opt/aog/bin"){
                    Ok(_) => {},
                    Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to copy aog binary").into()),
                }
            },
            Err(e) => log::error!("failed to get current exe path: {e}"),
        };
        match crate::aog::tools::systemctl_start("aog.service"){
            Ok(_) => {},
            Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to start aog as a service").into()),
        }
    }

    Ok(())
}


pub fn update_linux_service_file(args: aog::Args){
    let mut data = String::new();
    data.push_str("[Unit]\n");
    data.push_str("Description=aog\n");
    data.push_str("After=network.target\n");
    data.push_str("After=systemd-user-sessions.service\n");
    data.push_str("After=network-online.target\n\n");
    data.push_str("[Service]\n");
    if args.encrypt{
        data.push_str(format!("ExecStart=/opt/aog/bin/aog --max-threads {} --http-port {} --encrypt --key {}\n", args.max_threads, args.port, args.key).as_str());
    } else {
        data.push_str(format!("ExecStart=/opt/aog/bin/aog --max-threads {} --http-port {} --key {}\n", args.max_threads, args.port, args.key).as_str());
    }
    data.push_str("TimeoutSec=30\n");
    data.push_str("Restart=on-failure\n");
    data.push_str("RestartSec=30\n");
    data.push_str("StartLimitInterval=350\n");
    data.push_str("StartLimitBurst=10\n\n");
    data.push_str("[Install]\n");
    data.push_str("WantedBy=multi-user.target\n");
    if let Err(e) = std::fs::write("/lib/systemd/system/aog.service", data) {
        log::error!("Failed to write service file: {}", e);
    }
}
