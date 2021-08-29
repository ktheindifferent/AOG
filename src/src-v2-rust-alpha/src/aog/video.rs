use rscam::{Camera, Config};
use std::fs::File;
use std::io::Write;

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