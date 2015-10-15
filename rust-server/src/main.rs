mod webcam;
mod fountain;

use std::sync::mpsc::channel;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep_ms;
use std::net::TcpListener;
use fountain::Fountain;

const CAMERA: &'static str = "/dev/video0";
const SERVER: &'static str = "127.0.0.1:9997";

pub enum Status {
    NoClients,
    ClientsAvailable,
}

fn main() {
    let (camera, refresh_ms) = webcam::camera(CAMERA);

    println!("Refresh rate: {}ms", refresh_ms);

    let server = TcpListener::bind(SERVER).unwrap();

    let clients = Arc::new(Mutex::new(Fountain::new()));
    let listeners = clients.clone();

    let (report, status) = channel();
    let connections = Arc::new(Mutex::new((0u32, report)));
    let close_connections = connections.clone();
    let on_close = Arc::new(move || {
        let mut connections = close_connections.lock().unwrap();
        connections.0 -= 1;
        if connections.0 == 0 {
            connections.1.send(Status::NoClients).ok();
        }
    });

    thread::spawn(move || {
        loop {
            sleep_ms(refresh_ms);
            if let Ok(Status::NoClients) = status.try_recv() {
                while let Ok(stat) = status.recv() {
                    match stat {
                        Status::ClientsAvailable => break,
                        _ => continue,
                    }
                }
            }
            let frame = camera.capture().unwrap();
            listeners.lock().unwrap().send(frame);
        }
    });

    for conn in server.incoming() {
        match conn {
            Ok(mut conn) => {
                {
                    let mut connections = connections.lock().unwrap();
                    connections.0 += 1;
                    if connections.0 == 1 {
                        connections.1.send(Status::ClientsAvailable).ok();
                    }
                }
                let frames = clients.lock().unwrap().make_link();
                let close_handler = on_close.clone();
                thread::spawn(move || {
                    while let Ok(frame) = frames.recv() {
                        if let Err(_) = conn.write_all(&frame[..]) {
                            close_handler();
                        }
                    }
                });
            },
            Err(_) => continue,
        };
    }
}
