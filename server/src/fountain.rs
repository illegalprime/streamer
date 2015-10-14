use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::mpsc::channel;

pub struct Fountain<T> {
    senders: Vec<Sender<Arc<T>>>,
}

impl<T> Fountain<T>
where T: Sync {
    fn new() -> Self {
        Fountain {
            senders: Vec::new(),
        }
    }

    fn link(&mut self, sender: Sender<Arc<T>>) {
        self.senders.push(sender);
    }

    fn send(&self, data: T) {
        let data = Arc::new(data);
        for sender in self.senders.iter() {
            sender.send(data.clone());
        }
    }
}
