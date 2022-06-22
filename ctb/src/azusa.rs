use crate::chat::ChatMessagePacket;
use crate::rulesets::catch::CatchScore;
use crate::web_socket::{ConnectionStatus, WebSocket};
use crate::LogType;
use aether::log;
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerPacket {
    Pong,
    Echo(String),
    Chat(ChatMessagePacket),
    Connected {
        version: String,
    },
    Leaderboard {
        diff_id: u32,
        scores: Vec<CatchScore>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientPacket {
    Ping,
    Echo(String),
    Chat(String),
    Login(uuid::Uuid),
    Submit(CatchScore),
    RequestLeaderboard(u32),
    Goodbye,
}

pub struct Azusa {
    ws: WebSocket,
    connected: bool,
    logging_in: bool,

    token: uuid::Uuid,
}

impl Azusa {
    pub async fn new(token: uuid::Uuid) -> Self {
        let ws = WebSocket::new(vec!["ws://127.0.0.1:3012", "ws://azusa.nobbele.dev:3012"]);
        Azusa {
            ws,
            connected: false,
            logging_in: false,
            token,
        }
    }

    pub fn receive(&mut self) -> Vec<ServerPacket> {
        if self.ws.status() != ConnectionStatus::Connected && self.connected {
            self.set_connected(false);
        }

        if self.ws.status() == ConnectionStatus::Connected && !self.connected && !self.logging_in {
            self.send(&ClientPacket::Login(self.token));
            self.logging_in = true;
        }

        self.ws
            .poll()
            .unwrap_or_else(|e| {
                log!(LogType::Network, "{}", e);
                vec![]
            })
            .iter()
            .map(|data| bincode::deserialize(data).unwrap())
            .inspect(|packet: &ServerPacket| {
                log!(LogType::Network, "Got packet: '{:?}'", packet);
            })
            .collect()
    }

    pub fn set_connected(&mut self, status: bool) {
        log!(LogType::Network, "Azusa connected status: {}", status);
        self.connected = status;
        self.logging_in = false;
    }

    pub fn connected(&self) -> bool {
        self.connected
    }

    pub fn send(&self, message: &ClientPacket) {
        log!(LogType::Network, "Sending packet: {:?}", message);
        self.ws.send(bincode::serialize(message).unwrap());
    }
}

impl Drop for Azusa {
    fn drop(&mut self) {
        self.send(&ClientPacket::Goodbye);
    }
}
