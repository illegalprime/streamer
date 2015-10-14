extern crate websocket;

use std::thread;
use std::sync::mpsc::channel;
use websocket::{Server, Message, Sender, Receiver};
use websocket::header::WebSocketProtocol;

pub struct Pool {
	fountain: Arc<Mutex<Fountain<[u8]>>>,
}

impl Pool {
    fn start(url: &str) -> Self {
        let server = Server::bind(url).unwrap();
        let fountain_orig = Arc::new(Mutex::new(Fountain::new()));
        let fountain = fountain_orig;

        thread::spawn(move || {
            for connection in server {
                let (tx, rx) = channel();
                fountain.lock().unwrap().link(tx);
                while let Ok(message) = rx.recv() {
                    // TODO
                }
            }
        });
    }

    fn notify(&self, data: [u8]) {
        self.fountain.lock().unwrap().send(data);
    }
}


/* Handshake
 * 
 let request = connection.unwrap().read_request().unwrap(); // Get the request
 let headers = request.headers.clone(); // Keep the headers so we can check them
 
 request.validate().unwrap(); // Validate the request
 
 let mut response = request.accept(); // Form a response
 
 if let Some(&WebSocketProtocol(ref protocols)) = headers.get() {
     if protocols.contains(&("rust-websocket".to_string())) {
         // We have a protocol we want to use
         response.headers.set(WebSocketProtocol(vec!["rust-websocket".to_string()]));
     }
 }
 
 let mut client = response.send().unwrap(); // Send the response
*/
