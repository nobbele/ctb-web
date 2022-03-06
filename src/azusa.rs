use macroquad::prelude::*;
use quad_net::web_socket::WebSocket;
use serde::{Deserialize, Serialize};

use crate::chat::ChatMessagePacket;

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerPacket {
    Pong,
    Echo(String),
    Chat(ChatMessagePacket),
    Connected,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientPacket {
    Ping,
    Echo(String),
    Chat(String),
    Login,
}

pub struct Azusa {
    ws: WebSocket,
    connected: bool,
}

impl Azusa {
    pub async fn new() -> Self {
        let ws = WebSocket::connect("ws://127.0.0.1:3012").unwrap();
        while !ws.connected() {
            next_frame().await;
        }
        Azusa {
            ws,
            connected: false,
        }
    }

    pub fn receive(&mut self) -> Vec<ServerPacket> {
        std::iter::from_fn(|| self.ws.try_recv())
            .map(|data| bincode::deserialize(&data).unwrap())
            .inspect(|packet: &ServerPacket| {
                debug!("Got packet: '{:?}'", packet);
            })
            .collect()
    }

    pub fn set_connected(&mut self, status: bool) {
        println!("Azusa connected status: {}", status);
        self.connected = status;
    }

    pub fn connected(&self) -> bool {
        self.connected
    }

    pub fn send(&self, message: &ClientPacket) {
        self.ws.send_bytes(&bincode::serialize(message).unwrap());
    }
}
