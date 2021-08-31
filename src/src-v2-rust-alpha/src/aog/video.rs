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

pub fn init_all(){
    // Start video0 Thread
    thread::spawn(|| {
        init(format!("video0"));
    });

    // Start video1 Thread
    thread::spawn(|| {
        init(format!("video1"));
    });

    // Start video2 Thread
    thread::spawn(|| {
        init(format!("video2"));
    });
}

pub fn init(channel: String) {

    let device = Camera::new(format!("/dev/{}", channel).as_str());
    if device.is_ok() {
        let mut camera = device.unwrap();


        let hq_config = camera.start(&Config {
            interval: (1, 30),      // 30 fps.
            resolution: (1280, 720),
            format: b"MJPG",
            ..Default::default()
        });

        let mut camera_configured = false;

        if hq_config.is_ok() {
            hq_config.unwrap();
            camera_configured = true;
        } else {
            let lq_config = camera.start(&Config {
                interval: (1, 30),      // 30 fps.
                resolution: (320, 240),
                format: b"MJPG",
                ..Default::default()
            });
            if lq_config.is_ok() {
                lq_config.unwrap();
                camera_configured = true;
            }
        }
    
    
  
    
      
        if camera_configured{
            loop {
                let frame = camera.capture().unwrap();
                // println!("resolution: {:?}, timestamp: {:?}", frame.resolution, frame.get_timestamp());
        
                let mut file = File::create(&format!("/opt/aog/dat/{}.jpg", channel)).unwrap();
                file.write_all(&frame[..]).unwrap();
        
            }
        }

    }

}