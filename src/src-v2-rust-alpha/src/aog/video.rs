use rscam::{Camera, Config};
use std::fs::File;
use std::io::Write;

pub fn init(channel: String) {


    let mut camera = Camera::new(format!("/dev/{}", channel).as_str()).unwrap();

    camera.start(&Config {
        interval: (1, 30),      // 30 fps.
        resolution: (1280, 720),
        format: b"MJPG",
        ..Default::default()
    }).unwrap();


    
  
    
    loop {
        let frame = camera.capture().unwrap();
        // println!("resolution: {:?}, timestamp: {:?}", frame.resolution, frame.get_timestamp());

        let mut file = File::create(&format!("/opt/aog/dat/{}.jpg", channel)).unwrap();
        file.write_all(&frame[..]).unwrap();

    }
}