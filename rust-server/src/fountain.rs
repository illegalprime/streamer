use std::sync::mpsc::{Sender, Receiver};
use std::sync::Arc;
use std::sync::mpsc::channel;

#[derive(Clone)]
pub struct Fountain<T>
where T: Send + Sync {
    senders: Vec<Sender<Arc<T>>>,
}

impl<T> Fountain<T>
where T: Send + Sync {
    pub fn new() -> Self {
        Fountain {
            senders: Vec::new(),
        }
    }

    pub fn send(&self, data: T) {
        let data = Arc::new(data);
        for sender in self.senders.iter() {
            sender.send(data.clone()).unwrap();
        }
    }

    pub fn make_link(&mut self) -> Receiver<Arc<T>> {
        let (sender, receiver) = channel();
        self.senders.push(sender);
        receiver
    }
}
