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

use rscam::{Camera, Config};
use std::fs::File;
use std::io::Write;
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn init_all(){
    // Start video0 Thread
    thread::spawn(|| {
        init("video0".to_string());
    });

    // Start video1 Thread
    thread::spawn(|| {
        init("video1".to_string());
    });

    // Start video2 Thread
    thread::spawn(|| {
        init("video2".to_string());
    });
}

pub fn init(channel: String) {

    let device = Camera::new(format!("/dev/{}", channel).as_str());
    if device.is_ok() {
        let mut camera = match device {
            Ok(cam) => cam,
            Err(e) => {
                log::error!("Failed to open camera {}: {}", channel, e);
                return;
            }
        };


        let hq_config = camera.start(&Config {
            interval: (1, 30),      // 30 fps.
            resolution: (1280, 720),
            format: b"MJPG",
            ..Default::default()
        });

        let mut camera_configured = false;

        match hq_config {
            Ok(_) => {
                camera_configured = true;
            },
            Err(_) => {
                let lq_config = camera.start(&Config {
                    interval: (1, 30),      // 30 fps.
                    resolution: (320, 240),
                    format: b"MJPG",
                    ..Default::default()
                });
                if lq_config.is_ok() {
                    camera_configured = true;
                } else {
                    log::error!("Failed to configure camera {}", channel);
                }
            }
        }
    
    
  
    
      
        if camera_configured{
            let mut error_count = 0;
            const MAX_ERRORS: u32 = 10;
            
            loop {
                match camera.capture() {
                    Ok(frame) => {
                        // println!("resolution: {:?}, timestamp: {:?}", frame.resolution, frame.get_timestamp());
                        
                        if let Ok(mut file) = File::create(&format!("/opt/aog/dat/{}.jpg", channel)) {
                            let _ = file.write_all(&frame[..]);
                        }
                        
                        error_count = 0; // Reset error count on success
                    },
                    Err(e) => {
                        log::error!("Failed to capture frame from {}: {}", channel, e);
                        error_count += 1;
                        
                        if error_count >= MAX_ERRORS {
                            log::error!("Too many capture errors on {}, stopping camera thread", channel);
                            break;
                        }
                        
                        // Sleep before retry
                        thread::sleep(Duration::from_millis(100));
                    }
                }
                
                // Add small delay to prevent CPU spinning
                thread::sleep(Duration::from_millis(33)); // ~30fps
            }
        }

    }

}