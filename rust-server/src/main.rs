mod webcam;

use std::io::Write;
use std::net::TcpListener;

const CAMERA: &'static str = "/dev/video0";
const SERVER: &'static str = "127.0.0.1:9997";

fn main() {
    let (camera, refresh_ms) = webcam::camera(CAMERA);

    println!("Refresh rate: {}", refresh_ms);

    let server = TcpListener::bind(SERVER).unwrap();

    for conn in server.incoming() {
        match conn {
            Ok(mut conn) => {
                let frame = camera.capture().unwrap();
                conn.write_all(&frame[..]).unwrap();
                break;
            },
            Err(_) => continue,
        };
    }
}
