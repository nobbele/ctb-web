use crate::log_to;
use crate::screen::game::SharedGameData;

use crate::web_socket::{ConnectionStatus, WebSocket};
use crate::{chat::ChatMessagePacket, score::Score};
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
pub enum ServerPacket {
    Pong,
    Echo(String),
    Chat(ChatMessagePacket),
    Connected,
    Leaderboard { diff_id: u32, scores: Vec<Score> },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientPacket {
    Ping,
    Echo(String),
    Chat(String),
    Login(uuid::Uuid),
    Submit(Score),
    RequestLeaderboard(u32),
    Goodbye,
}

pub struct Azusa {
    ws: WebSocket,
    connected: bool,

    data: SharedGameData,
}

impl Azusa {
    pub async fn new(data: SharedGameData) -> Self {
        let ws = WebSocket::new(data.clone(), vec!["ws://127.0.0.1:3012", "ws://azusa.null"]);
        Azusa {
            ws,
            connected: false,

            data,
        }
    }

    pub fn receive(&mut self) -> Vec<ServerPacket> {
        self.ws
            .poll()
            .unwrap_or_else(|e| {
                log_to!(self.data.network, "{}", e);
                vec![]
            })
            .iter()
            .map(|data| bincode::deserialize(&data).unwrap())
            .inspect(|packet: &ServerPacket| {
                log_to!(self.data.network, "Got packet: '{:?}'", packet);
            })
            .collect()
    }

    pub fn set_connected(&mut self, status: bool) {
        log_to!(self.data.network, "Azusa connected status: {}", status);
        self.connected = status;
    }

    pub fn connected(&self) -> bool {
        self.ws.status() == ConnectionStatus::Connected
    }

    pub fn send(&self, message: &ClientPacket) {
        self.ws.send(bincode::serialize(message).unwrap());
    }
}

impl Drop for Azusa {
    fn drop(&mut self) {
        self.send(&ClientPacket::Goodbye);
    }
}
