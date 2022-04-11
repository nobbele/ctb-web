use super::{ConnectionStatus, WebSocketInterface};

enum Event {
    Connected(qws::Sender),
    Message(qws::Message),
    Error(qws::Error),
    Close {
        code: qws::CloseCode,
        reason: String,
    },
}

struct Client {
    out: qws::Sender,
    tx: flume::Sender<Event>,
}

impl qws::Handler for Client {
    fn on_open(&mut self, _shake: qws::Handshake) -> qws::Result<()> {
        self.tx.send(Event::Connected(self.out.clone())).unwrap();
        Ok(())
    }

    fn on_message(&mut self, msg: qws::Message) -> qws::Result<()> {
        self.tx.send(Event::Message(msg)).unwrap();
        Ok(())
    }

    fn on_close(&mut self, code: qws::CloseCode, reason: &str) {
        self.tx
            .send(Event::Close {
                code,
                reason: reason.to_owned(),
            })
            .unwrap();
    }

    fn on_error(&mut self, err: qws::Error) {
        let _ = self.tx.send(Event::Error(err));
    }
}

pub struct WebSocket {
    sender: Option<qws::Sender>,
    error: Option<qws::Error>,
    rx: flume::Receiver<Event>,
    send_queue: (flume::Sender<Vec<u8>>, flume::Receiver<Vec<u8>>),
}

impl WebSocketInterface for WebSocket {
    fn connect(addr: impl Into<String>) -> Self {
        let addr = addr.into();
        let (tx, rx) = flume::unbounded();
        std::thread::spawn(move || {
            if let Err(e) = qws::connect(addr, |out| Client {
                out,
                tx: tx.clone(),
            }) {
                tx.send(Event::Error(e)).unwrap();
            }
        });

        WebSocket {
            sender: None,
            error: None,
            rx,
            send_queue: flume::unbounded(),
        }
    }

    fn send(&self, data: Vec<u8>) {
        self.send_queue.0.send(data).unwrap();
    }

    fn poll(&mut self) -> Result<Vec<Vec<u8>>, String> {
        match self.status() {
            ConnectionStatus::Connected => {
                for msg in self.send_queue.1.drain() {
                    self.sender.as_ref().unwrap().send(msg).unwrap();
                }
            }
            ConnectionStatus::Error | ConnectionStatus::Disconnected => (),
        }

        let mut v = Vec::new();
        for ev in self.rx.drain() {
            match ev {
                Event::Connected(sender) => {
                    self.sender = Some(sender);
                }
                Event::Message(msg) => v.push(msg.into_data()),
                Event::Error(e) => return Err(format!("Web Socket Error: {}", e)),
                Event::Close { code, reason } => {
                    self.sender = None;
                    return Err(format!(
                        "Web Socket Closed with code {:?}. Reason: {}",
                        code, reason
                    ));
                }
            }
        }

        Ok(v)
    }

    fn status(&self) -> ConnectionStatus {
        if self.error.is_some() {
            ConnectionStatus::Error
        } else if self.sender.is_some() {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Disconnected
        }
    }
}
