#[derive(PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Error,
    Disconnected,
}

pub trait WebSocketInterface {
    fn connect(addr: impl Into<String>) -> Self;
    fn poll(&mut self) -> Vec<Vec<u8>>;
    fn send(&self, data: Vec<u8>);
    fn status(&self) -> ConnectionStatus;
}

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

//TODO Web

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::{Client, ConnectionStatus, Event, WebSocketInterface};
    use macroquad::prelude::*;

    pub struct NativeWebSocket {
        sender: Option<qws::Sender>,
        error: Option<qws::Error>,
        rx: flume::Receiver<Event>,
        send_queue: (flume::Sender<Vec<u8>>, flume::Receiver<Vec<u8>>),
    }

    impl WebSocketInterface for NativeWebSocket {
        fn connect(addr: impl Into<String>) -> NativeWebSocket {
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

            NativeWebSocket {
                sender: None,
                error: None,
                rx,
                send_queue: flume::unbounded(),
            }
        }

        fn send(&self, data: Vec<u8>) {
            self.send_queue.0.send(data).unwrap();
        }

        fn poll(&mut self) -> Vec<Vec<u8>> {
            match self.status() {
                ConnectionStatus::Connected => {
                    for msg in self.send_queue.1.drain() {
                        self.sender.as_ref().unwrap().send(msg).unwrap();
                    }
                }
                ConnectionStatus::Error | ConnectionStatus::Disconnected => (),
            }

            self.rx
                .drain()
                .filter_map(|ev| match ev {
                    Event::Connected(sender) => {
                        self.sender = Some(sender);
                        None
                    }
                    Event::Message(msg) => Some(msg.into_data()),
                    Event::Error(e) => {
                        info!("Web Socket Error: {}", e);
                        None
                    }
                    Event::Close { code, reason } => {
                        info!("Web Socket Closed with code {:?}. Reason: {}", code, reason);
                        self.sender = None;
                        None
                    }
                })
                .collect::<Vec<_>>()
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
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeWebSocket as WebSocket;

/*#[cfg(target_arch = "wasm32")]
pub(crate) mod js_web_socket {
    use std::net::ToSocketAddrs;

    use sapp_jsutils::JsObject;

    use crate::error::Error;

    pub struct WebSocket;

    extern "C" {
        fn ws_connect(addr: JsObject);
        fn ws_send(buffer: JsObject);
        fn ws_try_recv() -> JsObject;
        fn ws_is_connected() -> i32;
    }

    impl WebSocket {
        pub fn send_text(&self, text: &str) {
            unsafe { ws_send(JsObject::string(text)) };
        }

        pub fn send_bytes(&self, data: &[u8]) {
            unsafe { ws_send(JsObject::buffer(data)) };
        }

        pub fn try_recv(&mut self) -> Option<Vec<u8>> {
            let data = unsafe { ws_try_recv() };
            if data.is_nil() == false {
                let is_text = data.field_u32("text") == 1;
                let mut buf = vec![];
                if is_text {
                    let mut s = String::new();
                    data.field("data").to_string(&mut s);
                    buf = s.into_bytes();
                } else {
                    data.field("data").to_byte_buffer(&mut buf);
                }
                return Some(buf);
            }
            None
        }

        pub fn connected(&self) -> bool {
            unsafe { ws_is_connected() == 1 }
        }

        pub fn connect<A: ToSocketAddrs + std::fmt::Display>(addr: A) -> Result<WebSocket, Error> {
            unsafe { ws_connect(JsObject::string(&format!("{}", addr))) };

            Ok(WebSocket)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod pc_web_socket {
    use std::net::ToSocketAddrs;
    use std::sync::{mpsc, Mutex};

    pub struct WebSocket {
        sender: qws::Sender,
        rx: Mutex<mpsc::Receiver<Event>>,
    }

    enum Event {
        Connect(qws::Sender),
        Message(Vec<u8>),
    }

    struct Client {
        out: qws::Sender,
        thread_out: mpsc::Sender<Event>,
    }

    impl qws::Handler for Client {
        fn on_open(&mut self, _: qws::Handshake) -> qws::Result<()> {
            self.thread_out
                .send(Event::Connect(self.out.clone()))
                .unwrap();
            Ok(())
        }

        fn on_message(&mut self, msg: qws::Message) -> qws::Result<()> {
            self.thread_out
                .send(Event::Message(msg.into_data()))
                .unwrap();
            Ok(())
        }

        fn on_close(&mut self, code: qws::CloseCode, _reason: &str) {
            println!("closed {:?}", code);
        }

        fn on_error(&mut self, error: qws::Error) {
            println!("{:?}", error);
        }
    }

    impl WebSocket {
        pub fn connect<A: ToSocketAddrs + std::fmt::Display>(
            addr: A,
        ) -> Result<WebSocket, qws::Error> {
            let (tx, rx) = mpsc::channel();
            let ws_addr = format!("{}", addr);
            std::thread::spawn(move || {
                qws::connect(ws_addr, |out| Client {
                    out,
                    thread_out: tx.clone(),
                })
                .unwrap()
            });

            match rx.recv() {
                Ok(Event::Connect(sender)) => Ok(WebSocket {
                    sender,
                    rx: Mutex::new(rx),
                }),
                _ => panic!("Failed to connect websocket"),
            }
        }

        pub fn connected(&self) -> bool {
            true
        }

        pub fn try_recv(&mut self) -> Option<Vec<u8>> {
            self.rx
                .lock()
                .unwrap()
                .try_recv()
                .ok()
                .map(|event| match event {
                    Event::Message(msg) => msg,
                    _ => panic!(),
                })
        }

        pub fn send_text(&self, text: &str) {
            self.sender.send(qws::Message::text(text)).unwrap();
        }

        pub fn send_bytes(&self, data: &[u8]) {
            self.sender
                .send(qws::Message::Binary(data.to_vec()))
                .unwrap();
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use js_web_socket::WebSocket;

#[cfg(not(target_arch = "wasm32"))]
pub use pc_web_socket::WebSocket;
*/
