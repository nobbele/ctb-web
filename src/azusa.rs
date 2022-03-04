use macroquad::prelude::*;
use quad_net::web_socket::WebSocket;
use serde::{Deserialize, Serialize};

use crate::chat::ChatMessagePacket;

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerPacket {
    Echo(String),
    Chat(ChatMessagePacket),
    Connected,
    Pong,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientPacket {
    Echo(String),
    Chat(String),
    Ping,
}

pub struct Azusa {
    ws: WebSocket,
}

impl Azusa {
    pub async fn new() -> Self {
        let ws = WebSocket::connect("ws://127.0.0.1:3012").unwrap();
        while !ws.connected() {
            next_frame().await;
        }
        Azusa { ws }
    }

    pub fn receive(&mut self) -> impl Iterator<Item = ServerPacket> + '_ {
        std::iter::from_fn(|| self.ws.try_recv())
            .map(|data| bincode::deserialize(&data).unwrap())
            .inspect(|packet: &ServerPacket| {
                debug!("Got packet: '{:?}'", packet);
            })
    }

    pub fn send(&self, message: &ClientPacket) {
        self.ws.send_bytes(&bincode::serialize(message).unwrap());
    }
}
