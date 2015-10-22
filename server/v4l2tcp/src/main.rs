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
use v4l2_quick::{Dir, Pref, Constraints, ConfigSummary, Res};
use v4l2_quick::{Fmt, Speed, V4l2Result, Camera};

const CLIENT: Token = Token(0);
const SERVER: Token = Token(1);
const TIMEOUT: Token = Token(2);
const USAGE: &'static str = "<camera path> <listen addr>";

const FULL_IMAGE_PREFIX: [u8; 1] = [0x55];

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

struct CameraData {
    // Like /dev/video0
    path: String,
    // Camera handle
    handle: Option<Camera>,
    // Config for the fastest framerate
    fastest: ConfigSummary,
    // Config for the best quality
    best: ConfigSummary,
    // Milliseconds before next frame
    interval: u64,
}

struct CamServer {
    camera: CameraData,
    server: TcpListener,
    client: Option<Connection>,
    timeout: Option<Timeout>,
}

impl CamServer {
    fn new(cam_path: String, server: TcpListener) -> Result<Self, ()> {
        // Get the camera parameters with the best quality
        let want_quality = Constraints {
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
        // Get the configuration
        let quality = match v4l2_quick::configure(&cam_path, want_quality) {
            Ok(Some(config)) => config,
            _ => return Err(()),
        };
        // Get the camera parameters with the fastest framerate
        // But keep it above 640x480
        let want_framerate = Constraints {
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
        // Get the config
        let framerate = match v4l2_quick::configure(&cam_path, want_framerate) {
            Ok(Some(config)) => config,
            _ => return Err(()),
        };
        // Calculate how fast we should update
        let interval = framerate.interval;
        let refresh = ((interval.0 as f32 / interval.1 as f32) * 1000. + 0.5) as u64;
        // Lets start with the fast camera
        let mut camera = Camera::new(&cam_path).unwrap();
        v4l2_quick::start(&mut camera, &framerate).unwrap();
        // Cache configs for faster switching
        Ok(CamServer {
            server: server,
            client: None,
            timeout: None,
            camera: CameraData {
                handle: Some(camera),
                fastest: framerate,
                best: quality,
                interval: refresh,
                path: cam_path,
            },
        })
    }

    fn camera_fast(&mut self) -> V4l2Result<()> {
        // Get rid of the old camera
        self.camera.handle = None;
        // Make a new one with the 'fast' config
        let mut camera = try!(Camera::new(&self.camera.path));
        try!(v4l2_quick::start(&mut camera, &self.camera.fastest));
        self.camera.handle = Some(camera);
        Ok(())
    }

    fn camera_quality(&mut self) -> V4l2Result<()> {
        // Get rid of the old camera
        self.camera.handle = None;
        std::thread::sleep_ms(2000);
        // Make a new one with the 'fast' config
        let mut camera = try!(Camera::new(&self.camera.path));
        try!(v4l2_quick::start(&mut camera, &self.camera.best));
        self.camera.handle = Some(camera);
        Ok(())
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
                    client.stream.read_to_string(&mut buf).ok();
                    buf
                } else {
                    return;
                }
            } else {
                return;
            };
            println!("Message: {:?}", &message);
            match message.trim_right() {
                "capture" => {
                    // Find one that has really good quality
                    self.camera_quality().unwrap();
                    // Get a picture from the good camera
                    if let Ok(frame) = self.camera.handle.as_mut().unwrap().capture() {
                        if let Some(ref mut client) = self.client {
                            // Write the jpeg
                            client.stream.write_all(&frame[..]).ok();
                            // Write prefix so client knows this is high res
                            client.stream.write_all(&FULL_IMAGE_PREFIX).ok();
                        }
                    }
                    // Find the original, faster camera
                    self.camera_fast().unwrap();
                },
                "shutdown" => {
                    // Destroy everything
                    event_loop.shutdown();
                },
                "pause" => {
                    println!("Pausing.");
                    // Clear the timeout and subsequent frame captures
                    if let Some(timeout) = self.timeout {
                        event_loop.clear_timeout(timeout);
                        self.timeout = None;
                    }
                },
                "resume" => {
                    println!("Sarting to burst frames!");
                    // Start a new timer and capture frames
                    self.timeout = event_loop.timeout_ms(TIMEOUT, 0u64).ok();
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
                event_loop.timeout_ms(token, self.camera.interval).ok();
                return;
            }
            // Get a frame from the camera
            if let Ok(frame) = self.camera.handle.as_mut().unwrap().capture() {
                // Send it to the client
                if client.stream.write_all(&frame[..]).is_ok() {
                    // Guess how much longer we should wait until we go again
                    let used = time::precise_time_ns() - start;
                    let timeout = if used > self.camera.interval {
                        0
                    } else {
                        self.camera.interval - used
                    };
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
    let mut cams = CamServer::new(cam_path, server).unwrap();

    // Start event loop
    event_loop.register(&cams.server, SERVER).unwrap();
    event_loop.run(&mut cams).unwrap();
}

fn main() {
    let mut arguments = args();
    let program = arguments.next().unwrap();
    let camera = arguments.next();
    let server = arguments.next();
    match (camera, server) {
        (Some(c), Some(s)) => start(c, &s),
        _ => {
            writeln!(&mut stderr(), "Usage: {} {}", program, USAGE).ok();
            exit(1);
        }
    }
}
