

use crate::{
    log_to,
    screen::{game::SharedGameData},
};

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
    data: SharedGameData,

    sender: Option<qws::Sender>,
    tx: flume::Sender<Event>,
    rx: flume::Receiver<Event>,
    send_queue: (flume::Sender<Vec<u8>>, flume::Receiver<Vec<u8>>),

    connecting: bool,
}

impl WebSocketInterface for WebSocket {
    fn new(data: SharedGameData) -> Self {
        let (tx, rx) = flume::unbounded();

        WebSocket {
            data,
            sender: None,
            tx,
            rx,
            send_queue: flume::unbounded(),
            connecting: false,
        }
    }

    fn reset(&mut self) {
        (self.tx, self.rx) = flume::unbounded();
        self.connecting = false;
        self.sender = None;
    }

    fn connect(&mut self, addr: &str) {
        self.connecting = true;

        let addr = addr.to_owned();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            qws::connect(addr.as_str(), |out| Client {
                out,
                tx: tx.clone(),
            })
            .unwrap();
        });
    }

    fn poll(&mut self) -> Result<Vec<Vec<u8>>, String> {
        match self.status() {
            ConnectionStatus::Connected => {
                for msg in self.send_queue.1.drain() {
                    log_to!(
                        self.data.network,
                        "Sending message (length: {}) to socket.",
                        msg.len()
                    );
                    self.sender.as_ref().unwrap().send(msg).unwrap();
                }
            }
            _ => (),
        }

        let mut v = Vec::new();
        for ev in self.rx.drain() {
            // Assume to no longer be connecting if we received an event.
            self.connecting = false;

            match ev {
                Event::Connected(sender) => {
                    log_to!(self.data.network, "Socket connected.");
                    self.sender = Some(sender);
                }
                Event::Message(msg) => v.push(msg.into_data()),
                Event::Error(e) => {
                    return Err(format!("Web Socket Error: {}", e));
                }
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

    fn send(&self, data: Vec<u8>) {
        self.send_queue.0.send(data).unwrap();
    }

    fn status(&self) -> ConnectionStatus {
        if self.sender.is_some() {
            ConnectionStatus::Connected
        } else if self.connecting {
            ConnectionStatus::Connecting
        } else {
            ConnectionStatus::Disconnected
        }
    }
}
