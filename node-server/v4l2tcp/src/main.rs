extern crate v4l2_quick;

use std::io::Write;
use std::thread::sleep_ms;
use std::net::TcpListener;
use std::env::args;
use std::io::stderr;
use std::process::exit;

fn main() {
    let mut arguments = args();
    // Program name
    arguments.next();
    let camera = arguments.next();
    let server = arguments.next();
    match (camera, server) {
        (Some(c), Some(s)) => start(&c, &s),
        _ => {
            writeln!(&mut stderr(),
                "Usage: ./v4l2tcp <camera path> <listen addr>").ok();
            exit(1);
        }
    }
}

fn start(cam_path: &str, server_addr: &str) {
    let (camera, refresh_ms) = v4l2_quick::camera(cam_path).unwrap();
    let server = TcpListener::bind(server_addr).unwrap();
    println!("Started TCP Server on {} with camera {}.", cam_path, server_addr);

    for conn in server.incoming() {
        match conn {
            Ok(mut conn) => {
                loop {
                    sleep_ms(refresh_ms);
                    let frame = camera.capture().unwrap();
                    if conn.write_all(&frame[..]).is_err() {
                        break;
                    }
                }
            },
            Err(_) => continue,
        };
    }
}
