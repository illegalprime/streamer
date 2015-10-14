mod fountain;
mod webcam;
mod ws;

use std::thread::sleep_ms;
use std::fs::File;
use std::io::Write;

fn main() {
    let (camera, refresh_ms) = webcam::camera("/dev/video0");

    println!("Refresh rate: {}", refresh_ms);

    loop {
	    let frame = camera.capture().unwrap();
		let mut file = File::create("frame.jpg").unwrap();
		file.write_all(&frame[..]).unwrap();
		sleep_ms(refresh_ms);
    }
}
