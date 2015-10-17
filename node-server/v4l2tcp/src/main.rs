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
use mio::Timeout;
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
struct Connection {
    stream: TcpStream,
    can_write: bool,
}

impl Connection {
    fn new(stream: TcpStream) -> Self {
        Connection {
            stream: stream,
            can_write: false,
        }
    }

    fn reregister(&self, event_loop: &mut EventLoop<CamServer>) {
        event_loop.reregister(
            &self.stream,
            CLIENT,
            EventSet::readable() | EventSet::error() | EventSet::hup(),
            PollOpt::level()).unwrap();
    }
}

struct CamServer {
    camera: Camera,
    interval: u64,
    server: TcpListener,
    client: Option<Connection>,
    timeout: Option<Timeout>,
    cam_path: String,
}

impl CamServer {
    fn new(cam_path: String, server: TcpListener) -> Self {
        let (camera, refresh) = CamServer::camera_fast(&cam_path).unwrap();
        CamServer {
            camera: camera,
            interval: refresh,
            server: server,
            client: None,
            timeout: None,
            cam_path: cam_path,
        }
    }

    fn camera_fast(path: &str) -> Result<(Camera, u64), ()> {
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
        let (camera, config) = v4l2_quick::camera(path, constraints).unwrap();
        let config = match config {
            Some(c) => c,
            None => return Err(()),
        };
        // Find the timeout for the frame of the camera
        let interval = config.interval;
        let refresh_ms = ((interval.0 as f32 / interval.1 as f32) * 1000. + 0.5) as u64;
        Ok((camera, refresh_ms))
    }

    fn camera_quality(path: &str) -> Result<Camera, ()> {
        // Best quality!
        let constraints = Constraints {
            formats: Some(Fmt {
                emulate: Pref::NoPreference,
                compress: Pref::DoNotPrefer,
                priorities: Some(vec![b"MJPG"]),
            }),
            resolutions: Some(Res {
                dir: Dir::Highest,
                limit: None,
            }),
            speeds: None,
            .. Default::default()
        };
        // Get the camera
        let found = v4l2_quick::camera(path, constraints);
        match found {
            Ok((cam, config)) => {
                if config.is_some() {
                    Ok(cam)
                } else {
                    Err(())
                }
            },
            Err(_) => Err(()),
        }
    }
}

impl Handler for CamServer {
    type Timeout = Token;
    type Message = ();

    fn ready(&mut self, event_loop: &mut EventLoop<Self>, token: Token, events: EventSet) {
        println!("LOOP: {:?} with {:?}", token, &events);
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
                        let connection = Connection::new(stream);
                        // Register this with the event loop
                        event_loop.register_opt(
                            &connection.stream,
                            CLIENT,
                            EventSet::all(),
                            PollOpt::edge()).unwrap();
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
                    // We can write to this thing! Remember it!
                    client.can_write = true;
                }
                if events.is_readable() {
                    // read message into a buffer
                    let mut buf = String::new();
                    // Return data read
                    client.stream.read_to_string(&mut buf).map(|_| buf)
                } else {
                    return;
                }
            } else {
                return;
            };
            println!("Message: {:?}", &message);
            match message.as_ref().map(|s| s as &str) {
                Ok("capture") => {
                    // Stop the current camera
                    self.camera.stop().ok();
                    // Find one that has really good quality
                    let camera = CamServer::camera_quality(&self.cam_path).unwrap();
                    // Get a picture from the good camera
                    if let Ok(frame) = camera.capture() {
                        if let Some(ref mut client) = self.client {
                            // Send the picture to the client
                            client.stream.write_all(&frame[..]).ok();
                        }
                    }
                    // Find the original, faster camera
                    let (old_cam, _) = CamServer::camera_fast(&self.cam_path).unwrap();
                    // set our camera to the old one.
                    self.camera = old_cam;
                },
                Ok("shutdown") => {
                    // Destroy everything
                    event_loop.shutdown();
                },
                Ok("pause") => {
                    // Clear the timeout and subsequent frame captures
                    if let Some(timeout) = self.timeout {
                        event_loop.clear_timeout(timeout);
                        self.timeout = None;
                    }
                },
                Ok("resume") => {
                    // Start a new timer and capture frames
                    self.timeout = event_loop.timeout_ms(TIMEOUT, 0u64).ok();
                },
                Err(_) => {
                    // Remove the client
                    // self.client = None;
                },
                _ => return,
            };
        }
    }

    fn timeout(&mut self, event_loop: &mut EventLoop<Self>, token: Self::Timeout) {
        // Record the time so we have an estimate of how long this takes
        let start = time::precise_time_ns();
        // Check if we have a client, if not implicitly stop the timeout cycle
        if let Some(ref mut client) = self.client {
            // Check if we can write to the client
            if !client.can_write {
                // Wait until we can write
                event_loop.timeout_ms(token, self.interval).ok();
                return;
            }
            // Get a frame from the camera
            if let Ok(frame) = self.camera.capture() {
                // Send it to the client
                if client.stream.write_all(&frame[..]).is_ok() {
                    // Guess how much longer we should wait until we go again
                    let used = time::precise_time_ns() - start;
                    let timeout = if used > self.interval {
                        0
                    } else {
                        self.interval - used
                    };
                    println!("FRAME!");
                    // If sending went ok, do it again soon
                    self.timeout = event_loop.timeout_ms(token, timeout).ok();
                }
            }
        }
    }
}

fn start(cam_path: String, server_addr: &str) {
    // Create the TCP Server
    let address = SocketAddr::from_str(server_addr).unwrap();
    let server = TcpListener::bind(&address).unwrap();

    // Make an event loop
    let mut event_loop = EventLoop::configured(EventLoopConfig {
        timer_tick_ms: 1u64,
        .. Default::default()
    }).unwrap();

    // Server
    let mut cams = CamServer::new(cam_path, server);

    // Start event loop
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
        (Some(c), Some(s)) => start(c, &s),
        _ => {
            writeln!(&mut stderr(), "{}", USAGE).ok();
            exit(1);
        }
    }
}
