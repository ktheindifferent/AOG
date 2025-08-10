// ████████ ██   ██  █████  ██       █████  ███    ███ ██    ██ ███████ 
//    ██    ██   ██ ██   ██ ██      ██   ██ ████  ████ ██    ██ ██      
//    ██    ███████ ███████ ██      ███████ ██ ████ ██ ██    ██ ███████ 
//    ██    ██   ██ ██   ██ ██      ██   ██ ██  ██  ██ ██    ██      ██ 
//    ██    ██   ██ ██   ██ ███████ ██   ██ ██      ██  ██████  ███████                                                                             
// Copyright 2021-2023 The Open Sam Foundation (OSF)
// Developed by Caleb Mitchell Smith (PixelCoda)
// Licensed under GPLv3....see LICENSE file.



use std::fs::File;
use std::io::Write;
use std::process::{Command, Stdio};
use error_chain::error_chain;
use sha2::{Sha256, Digest};
use std::{io, fs};


error_chain! {
    foreign_links {
        Io(std::io::Error);
        // Hound(hound::Error);
    }
}


pub fn hash_check(file_path: &str) -> Result<String>{
    let f = File::open(file_path)?;

    let x = f.metadata().unwrap().len();
    log::warn!("File size: {}", x);
    log::warn!("hasging file: {}", file_path);
    let mut hasher = Sha256::new();
    let mut file = fs::File::open(file_path)?;

    let _bytes_written = io::copy(&mut file, &mut hasher)?;
    let hashh: String = format!("{:X}", hasher.finalize());
    log::warn!("done hasging file: {}", file_path);
    return Ok(hashh.to_string().to_lowercase());
}


pub fn get_file_size(file_path: &str) -> Result<i64>{
    let f = File::open(file_path)?;
    let x = f.metadata()?.len();
    return Ok(x as i64);
}


pub fn dbash(command: &str) -> Result<String>{
    let child = Command::new("/usr/bin/sudo")
    .arg("-u")
    .arg("sam")
    .arg("/bin/bash")
    .arg("-c")
    .arg(command)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn bash(command: &str) -> Result<String>{
    let child = Command::new("/bin/bash")
    .arg("-c")
    .arg(command)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn brew_install(package: &str) -> Result<String>{

    let child = Command::new("/usr/bin/sudo")
    .arg("-u")
    .arg("sam")
    .arg("/opt/homebrew/bin/brew")
    .arg("install")
    .arg(package)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn untar(file_path: &str) -> Result<String>{

    let child = Command::new("/bin/tar")
    .arg("-xf")
    .arg(file_path)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn apt_install(package: &str) -> Result<String>{

    let child = Command::new("/bin/apt")
    .arg("install")
    .arg(package)
    .arg("-y")
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn dnf_install(package: &str) -> Result<String>{

    let child = Command::new("/bin/dnf")
    .arg("install")
    .arg(package)
    .arg("-y")
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn brew_uninstall(package: &str) -> Result<String>{
    let child = Command::new("/usr/bin/sudo")
    .arg("-u")
    .arg("sam")
    .arg("/opt/homebrew/bin/brew")
    .arg("uninstall")
    .arg(package)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn ln(path: &str, link: &str) -> Result<String>{
    let child = Command::new("/bin/ln")
    .arg("-s")
    .arg(path)
    .arg(link)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn mv(source: &str, destination: &str) -> Result<String>{
    let child = Command::new("/bin/mv")
    .arg(source)
    .arg(destination)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn cp(source: &str, destination: &str) -> Result<String>{
    let child = Command::new("/bin/cp")
    .arg(source)
    .arg(destination)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn launchd_bootstrap(destination: &str) -> Result<String>{
    let child = Command::new("/bin/launchctl")
    .arg("bootstrap")
    .arg("system")
    .arg(destination)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn launchd_bootout(destination: &str) -> Result<String>{
    let child = Command::new("/bin/launchctl")
    .arg("bootout")
    .arg("system")
    .arg(destination)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn launchd_enable(destination: &str) -> Result<String>{
    let child = Command::new("/bin/launchctl")
    .arg("enable")
    .arg(destination)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn launchd_kickstart(destination: &str) -> Result<String>{
    let child = Command::new("/bin/launchctl")
    .arg("kickstart")
    .arg("-kp")
    .arg(destination)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn systemctl_reload() -> Result<String>{
    let child = Command::new("/bin/systemctl")
    .arg("daemon-reload")
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn systemctl_start(service_name: &str) -> Result<String>{
    let child = Command::new("/bin/systemctl")
    .arg("start")
    .arg(service_name)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn systemctl_stop(service_name: &str) -> Result<String>{
    let child = Command::new("/bin/systemctl")
    .arg("stop")
    .arg(service_name)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn systemctl_enable(service_name: &str) -> Result<String>{
    let child = Command::new("/bin/systemctl")
    .arg("enable")
    .arg(service_name)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn rm(path: &str) -> Result<String>{
    let child = Command::new("/bin/rm")
    .arg(path)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn rmd(path: &str) -> Result<String>{
    let child = Command::new("/bin/rm")
    .arg("-rf")
    .arg(path)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}




pub fn whisper(model: &str, file_path: &str) -> Result<String>{
    
    
    let child = Command::new("/opt/thalamus/bin/whisper")
    .arg("-m")
    .arg(format!("/opt/thalamus/models/ggml-{}.bin", model))
    .arg("-f")
    .arg(format!("{}.16.wav", file_path))
    .arg("-otxt")
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
    
}

pub fn whisper_owts(model: &str, file_path: &str) -> Result<String>{
    
    
    let child = Command::new("/opt/thalamus/bin/whisper")
    .arg("-m")
    .arg(format!("/opt/thalamus/models/ggml-{}.bin", model))
    .arg("-f")
    .arg(format!("{}.16.wav", file_path))
    .arg("-fp")
    .arg("/opt/thalamus/fonts/courier.ttf")
    .arg("-owts")
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
    
}
            

pub fn mkdir(apath: &str) -> Result<String>{
    if std::path::Path::new(apath).exists(){
        return Ok(format!("{} already exists", apath));  
    }

    let child = Command::new("/bin/mkdir")
    .arg(apath)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
}

pub fn fix_permissions(apath: &str) -> Result<String>{
    let child = Command::new("/bin/chmod")
    .arg("-R")
    .arg("777")
    .arg(apath)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string()); 
}

pub fn mark_as_executable(apath: &str) -> Result<String>{
    let child = Command::new("/bin/chmod")
    .arg("+x")
    .arg(apath)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string()); 
}

pub fn sh(script: &str) -> Result<String>{
    let child = Command::new("/bin/sh")
    .arg(script)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string()); 
}

pub fn srgan(input: &str, output: &str) -> Result<String>{
    let child = Command::new("/opt/bin/thalamus/srgan")
    .arg(input)
    .arg(output)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string()); 
}


pub fn download(file_path: &str, url: &str) -> Result<bool>{
    let child = Command::new("/opt/thalamus/bin/wget")
    .arg("-O")
    .arg(file_path)
    .arg(url)
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let _output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(true);
}

pub fn wav_to_16000(input: String) -> Result<String>{
    let child = Command::new("/opt/thalamus/bin/ffmpeg")
    .arg("-y")
    .arg("-i")
    .arg(format!("{}", input))
    .arg("-ar")
    .arg("16000")
    .arg("-ac")
    .arg("1")
    .arg("-c:a")
    .arg("pcm_s16le")
    .arg(format!("{}.16.wav", input))
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
}





pub fn touch(path: String) -> Result<()>{
    let mut output = File::create(path.as_str())?;
    write!(output, "")?;
    Ok(())
}

// ./main -m ./models/7B/ggml-model-q4_0.bin -p "Building a website can be done in 10 simple steps:" -n 512


pub fn llama(model: &str, prompt: &str) -> Result<String>{
    let child = Command::new("/opt/thalamus/bin/llama")
    .arg("-m")
    .arg(format!("/opt/thalamus/models/llama/{}/ggml-model-q4_0.bin", model))
    .arg("-p")
    .arg(format!("\"{}\"", prompt))
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to execute child");


    let output = child
    .wait_with_output()
    .expect("failed to wait on child");

    return Ok(String::from_utf8_lossy(&output.stdout).to_string());    
    
}


pub fn find_mimetype(filename: &String) -> String{
    let parts : Vec<&str> = filename.split('.').collect();
    let res = match parts.last() {
            Some(v) =>
                match *v {
                    "aac" => "audio/aac".to_string(),
                    "abw" => "application/x-abiword".to_string(),
                    "arc" => "application/x-freearc".to_string(),
                    "avi" => "video/x-msvideo".to_string(),
                    "azw" => "application/vnd.amazon.ebook".to_string(),
                    "bin" => "application/octet-stream".to_string(),
                    "bmp" => mime::IMAGE_BMP.to_string(),
                    "bz" => "application/x-bzip".to_string(),
                    "bz2" => "application/x-bzip2".to_string(),
                    "csh" => "application/x-csh".to_string(),
                    "css" => mime::TEXT_CSS.to_string(),
                    "csv" => "text/csv".to_string(),
                    "deb" => "application/vnd.debian.binary-package".to_string(),
                    "doc" => "application/msword".to_string(),
                    "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
                    "eot" => "application/vnd.ms-fontobject".to_string(),
                    "epub" => "application/epub+zip".to_string(),
                    "gz" => "application/gzip".to_string(),
                    "gif" => mime::IMAGE_GIF.to_string(),
                    "htm" => mime::TEXT_HTML.to_string(),
                    "html" => mime::TEXT_HTML.to_string(),
                    "ico" => "image/vnd.microsoft.icon".to_string(),
                    "ics" => "text/calendar".to_string(),
                    "jar" => "application/java-archive".to_string(),
                    "jpg" => mime::IMAGE_JPEG.to_string(),
                    "jpeg" => mime::IMAGE_JPEG.to_string(),
                    "js" => mime::TEXT_JAVASCRIPT.to_string(),
                    "json" => mime::APPLICATION_JSON.to_string(),
                    "jsonld" => "application/ld+json".to_string(),
                    "mid" => "audio/midi audio/x-midi".to_string(),
                    "midi" => "audio/midi audio/x-midi".to_string(),
                    "mjs" => "text/javascript".to_string(),
                    "mp3" => "audio/mpeg".to_string(),
                    "mp4" => "video/mp4".to_string(),
                    "mpeg" => "video/mpeg".to_string(),
                    "mpkg" => "application/vnd.apple.installer+xml".to_string(),
                    "odp" => "application/vnd.oasis.opendocument.presentation".to_string(),
                    "ods" => "application/vnd.oasis.opendocument.spreadsheet".to_string(),
                    "odt" => "application/vnd.oasis.opendocument.text".to_string(),
                    "oga" => "audio/ogg".to_string(),
                    "ogv" => "video/ogg".to_string(),
                    "ogg" => "audio/ogg".to_string(),
                    "ogx" => "application/ogg".to_string(),
                    "opus" => "audio/opus".to_string(),
                    "otf" => "font/otf".to_string(),
                    "png" => "image/png".to_string(),
                    "pdf" => "application/pdf".to_string(),
                    "php" => "application/x-httpd-php".to_string(),
                    "ppt" => "application/vnd.ms-powerpoint".to_string(),
                    "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string(),
                    "rar" => "application/vnd.rar".to_string(),
                    "rtf" => "application/rtf".to_string(),
                    "sh" => "application/x-sh".to_string(),
                    "svg" => "image/svg+xml".to_string(),
                    "swf" => "application/x-shockwave-flash".to_string(),
                    "tar" => "application/x-tar".to_string(),
                    "tif" => "image/tiff".to_string(),
                    "tiff" => "image/tiff".to_string(),
                    "ts" => "video/mp2t".to_string(),
                    "ttf" => "font/ttf".to_string(),
                    "txt" => "text/plain".to_string(),
                    "vsd" => "application/vnd.visio".to_string(),
                    "wasm" => "application/wasm".to_string(),
                    "wav" => "audio/wav".to_string(),
                    "weba" => "audio/webm".to_string(),
                    "webm" => "video/webm".to_string(),
                    "webp" => "image/webp".to_string(),
                    "woff" => "font/woff".to_string(),
                    "woff2" => "font/woff2".to_string(),
                    "xhtml" => "application/xhtml+xml".to_string(),
                    "xls" => "application/vnd.ms-excel".to_string(),
                    "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
                    "xml" => "text/xml".to_string(),
                    "xul" => "application/vnd.mozilla.xul+xml".to_string(),
                    "zip" => "application/zip".to_string(),
                    "3gp" => "video/3gpp".to_string(),
                    "3gp2" => "video/3gpp2".to_string(),
                    "7z" => "application/x-7z-compressed".to_string(),                   
                    &_ => mime::TEXT_PLAIN.to_string(),
                },
            None => mime::TEXT_PLAIN.to_string(),
        };
    res
}