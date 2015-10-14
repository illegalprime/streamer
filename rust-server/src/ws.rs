extern crate websocket;

use std::thread;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::mpsc::{TryRecvError, RecvError};
use std::sync::{Arc, Mutex};
use self::websocket::Server;
use self::websocket::header::WebSocketProtocol;
use self::websocket::stream::WebSocketStream;
use self::websocket::server::Connection as WsConnection;
use self::websocket::client::Client as WsClient;
use self::websocket::dataframe::DataFrame;
use self::websocket::server::sender::Sender as WsSender;
use self::websocket::server::receiver::Receiver as WsReceiver;
use self::websocket::result::WebSocketError;
use self::websocket::ws::message::Message;
use super::fountain::Fountain;

pub const PROTOCOL: &'static str = "jpeg-meta";

type Client<'d, 'r> = WsClient<DataFrame<'d>, WsSender<WebSocketStream>, WsReceiver<'r, WebSocketStream>>;
type Connection = WsConnection<WebSocketStream, WebSocketStream>;

pub enum Status {
    NoClients,
    ClientsAvailable,
}

macro_rules! break_if_err(
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(_) => continue,
    })
);

pub struct Pool<M>
where M: for<'a> Message<'a, DataFrame<'a>> + Send + Sync {
    links: Arc<Mutex<Vec<Sender<Arc<M>>>>>,
    status: Receiver<Status>,
}

impl<M> Pool<M>
where M: for<'a> Message<'a, DataFrame<'a>> + Send + Sync {
    pub fn start(url: &str, protocol: &'static str) -> Self {
        let server = Server::bind(url).unwrap();
        let links = Arc::new(Mutex::new(Vec::new()));

        let (report_status, view_status) = channel();
        Self::spawn_server(server, protocol, Arc::new(Mutex::new(report_status)), links.clone());

        Pool {
            links: links,
            status: view_status,
        }
    }

    pub fn notify(&self, data: M) {
        let data = Arc::new(data);
        // TODO: Remove broken channels here?
        for sender in self.links.lock().unwrap().iter() {
            sender.send(data.clone());
        }
    }

    pub fn status(&self) -> Result<Status, TryRecvError> {
        self.status.try_recv()
    }

    pub fn status_block(&self) -> Result<Status, RecvError> {
        self.status.recv()
    }

    fn spawn_client<'a>(client: Client<'a, 'a>,
                        channel: Receiver<Arc<M>>,
                        status: Arc<Mutex<Sender<Status>>>,
                        clients: Arc<Mutex<u32>>)
    {
        thread::spawn(move || {
            while let Ok(message) = channel.recv() {
                if let Err(_) = client.send_message(&*message) {
                    let remaining = clients.lock().unwrap();
                    *remaining -= 1;
                    if *remaining == 0 {
                        status.lock().unwrap().send(Status::NoClients).unwrap();
                    }
                }
            }
        });
    }

    fn spawn_server(server: Server,
                    proto: &'static str,
                    status: Arc<Mutex<Sender<Status>>>,
                    links: Arc<Mutex<Vec<Sender<Arc<M>>>>>)
    {
        let clients = Arc::new(Mutex::new(0u32));
        thread::spawn(move || {
            for connection in server {
                {
                    let remaining = clients.lock().unwrap();
                    *remaining += 1;
                    if *remaining == 1 {
                        status.lock().unwrap().send(Status::ClientsAvailable).unwrap();
                    }
                }
                let connection = break_if_err!(connection);
                let (tx, rx) = channel();
                let client = break_if_err!(Self::handshake(connection, proto));
                links.lock().unwrap().push(tx);
                Self::spawn_client(client, rx, status.clone(), clients.clone());
            }
        });
    }

    fn handshake(conn: Connection, proto: &str) -> Result<Client, WebSocketError> {
        let request = conn.read_request().unwrap(); // Get the request
        let headers = request.headers.clone(); // Keep the headers so we can check them
        request.validate().unwrap(); // Validate the request
        let mut response = request.accept(); // Form a response

        if let Some(&WebSocketProtocol(ref protocols)) = headers.get() {
            if protocols.contains(&(proto.to_string())) {
                // We have a protocol we want to use
                response.headers.set(WebSocketProtocol(vec![proto.to_string()]));
            }
        }

        response.send() // Send the response, get a client
    }
}
