extern crate v4l2_quick;
extern crate mio;
extern crate time;

use std::str::FromStr;
use std::io::Read;
use std::net::SocketAddr;
use std::io::Write;
use std::env::args;
use std::io::stderr;
use std::process::exit;
use mio::EventLoop;
use mio::EventLoopConfig;
use mio::Handler;
use mio::EventSet;
use mio::PollOpt;
use mio::Token;
use mio::tcp::TcpListener;
use mio::tcp::TcpStream;
use v4l2_quick::*;

const CLIENT: Token = Token(0);
const SERVER: Token = Token(1);
const TIMEOUT: Token = Token(2);
const USAGE: &'static str = "Usage: ./v4l2tcp <camera path> <listen addr>";

#[derive(Debug)]
struct Connection(TcpStream);

impl Connection {
    fn reregister(&self, event_loop: &mut EventLoop<CamServer>) {
        event_loop.reregister(
            &self.0,
            CLIENT,
            EventSet::readable() | EventSet::error() | EventSet::hup(),
            PollOpt::edge());
    }
}

struct CamServer {
    camera: Camera,
    interval: u64,
    server: TcpListener,
    client: Option<Connection>,
}

impl Handler for CamServer {
    type Timeout = Token;
    type Message = ();

    fn ready(&mut self, event_loop: &mut EventLoop<Self>, token: Token, events: EventSet) {
        println!("token: {:?}, events: {:?}", &token, &events);
        // If we are handling server events
        if token == SERVER {
            // If the server is readable
            // then a new client is ready to connect
            // although we must only accept one client at a time
            if events.is_readable() {
                match self.server.accept() {
                    Ok(Some(stream)) => {
                        // Only accept one client at a time!
                        if self.client.is_some() {
                            return;
                        }
                        // Register this stream
                        // We only want to know when there is data available from the
                        // client
                        let connection = Connection(stream);
                        // Register this with the event loop
                        event_loop.register_opt(
                            &connection.0,
                            CLIENT,
                            EventSet::all(),
                            PollOpt::edge());
                        // Save the client
                        self.client = Some(connection);
                    },
                    _ => return,
                };
            }
        } else if token == CLIENT {
            // The client ran into an error or
            // hung up on us
            if events.is_hup() || events.is_error() {
                // Remove the disconnected client
                self.client = None;
                return;
            }
            // Get message or return if there's none
            let message = if let Some(ref mut client) = self.client.as_mut() {
                // Put this client back in the event loop
                client.reregister(event_loop);
                if events.is_writable() {
                    // We can write to this thing! Send the frames!
                    event_loop.timeout_ms(TIMEOUT, 0u64);
                }
                if events.is_readable() {
                    // read message into a buffer
                    let mut buf = String::new();
                    // Return data read
                    client.0.read_to_string(&mut buf).map(|_| buf)
                } else {
                    return;
                }
            } else {
                return;
            };
            match message.as_ref().map(|s| s as &str) {
                Ok("picture") => { /* TODO */ },
                Ok("shutdown") => { /* TODO */ },
                Err(_) => {
                    // Remove the client if it can no
                    // longer be read from
                    self.client = None;
                }
                _ => return,
            };
        }
    }

    fn timeout(&mut self, event_loop: &mut EventLoop<Self>, timeout: Self::Timeout) {
        // Check if we have a client, if not implicitly stop the timeout cycle
        if let Some(ref mut client) = self.client {
            // Get a frame from the camera
            if let Ok(frame) = self.camera.capture() {
                // Send it to the client
                if client.0.write_all(&frame[..]).is_ok() {
                    // If it went ok, do it again soon
                    println!("FRAME {}", time::precise_time_ns());
                    event_loop.timeout_ms(timeout, 33u64);
                } else {
                    // TODO: Remove client
                }
            }
        }
    }
}

fn start(cam_path: &str, server_addr: &str) {
    // Best configuration for highest Framerate
    // without going below 640x480
    let constraints = Constraints {
        formats: Some(Fmt {
            emulate: Pref::DoNotPrefer,
            compress: Pref::Prefer,
            priorities: Some(vec![b"MJPG"]),
        }),
        resolutions: Some(Res {
            dir: Dir::Lowest,
            limit: Some((640, 480)),
        }),
        speeds: Some(Speed {
            dir: Dir::Highest,
            limit: None,
        }),
        .. Default::default()
    };
    // Get the camera
    let (camera, config) = v4l2_quick::camera(cam_path, constraints).unwrap();
    let config = match config {
        Some(c) => c,
        None => panic!("Configuration not found!"),
    };
    // Create the TCP Server
    let address = SocketAddr::from_str(server_addr).unwrap();
    let server = TcpListener::bind(&address).unwrap();
    // Find the timeout for the frame of the camera
    let interval = config.interval;
    let refresh_ms = ((interval.0 as f32 / interval.1 as f32) * 1000. + 0.5) as u64;

    // Make an event loop
    let mut event_loop = EventLoop::configured(EventLoopConfig {
        timer_tick_ms: 1u64,
        .. Default::default()
    }).unwrap();
    // Print status
    println!("Started TCP Server on {} with camera {}.", cam_path, server_addr);
    println!("Camera settings: {:?}", &config);
    println!("Refresh rate: {}ms", refresh_ms);

    // Server
    let mut cams = CamServer {
        camera: camera,
        interval: refresh_ms,
        server: server,
        client: None,
    };

    event_loop.register(&cams.server, SERVER).unwrap();
    event_loop.run(&mut cams).unwrap();
}

fn main() {
    let mut arguments = args();
    // First arg is program name
    arguments.next();
    let camera = arguments.next();
    let server = arguments.next();
    match (camera, server) {
        (Some(c), Some(s)) => start(&c, &s),
        _ => {
            writeln!(&mut stderr(), "{}", USAGE).ok();
            exit(1);
        }
    }
}
