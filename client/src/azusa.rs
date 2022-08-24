use crate::chat::ChatMessagePacket;
use crate::rulesets::catch::CatchScore;
use crate::web_socket::{ConnectionStatus, WebSocket};
use crate::LogType;
use aether::log;
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

/// Packet sent from Azusa, towards the game client
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerPacket {
    /// Response to [`ClientPacket::Ping`]
    Pong,
    /// Response to [`ClientPacket::Echo`], returning the same value sent by the client. Used for testing
    Echo(String),
    /// Receive a chat message from the global chat
    Chat(ChatMessagePacket),
    /// Inform the client they have been connected and logged in
    Connected { version: String },
    /// Response to [`ClientPacket::RequestLeaderboard`]
    Leaderboard {
        diff_id: u32,
        scores: Vec<CatchScore>,
    },
}

/// Packet sent from the game client, towards Azusa
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientPacket {
    /// Ping Azusa, used to check connectivity
    Ping,
    /// Used for testing
    Echo(String),
    /// Sends a chat message to the global chat
    Chat(String),
    /// Authenticate with the server using UUID. To login with username and password, use the website API
    Login(uuid::Uuid),
    /// Submit a score to the leaderboard
    Submit(CatchScore),
    /// Request the leaderboard for given difficulty id. Reponse given via [`ServerPacket::Leaderboard`]
    RequestLeaderboard(u32),
    /// Inform Azusa we are quitting
    Goodbye,
}

/// Manages things related to communication with Azusa (game server)
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

    /// Drain all packets that were queued since last call.
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

    /// Set whether or not we are connected.
    ///
    /// This is managed outside of this object because this needs to be set when we receive [`ServerPacket::Connected`] or if pinging failed, which are both handled in the Game object.
    ///
    /// It would be possible to view every [`ServerPacket`] received in [`Self::receive`] but I think this approach is a bit cleaner albeit being a bit confusing.
    pub fn set_connected(&mut self, status: bool) {
        log!(LogType::Network, "Azusa connected status: {}", status);
        self.connected = status;
        self.logging_in = false;
    }

    /// Returns the value of the `connected` field
    pub fn connected(&self) -> bool {
        self.connected
    }

    /// Sends a message to Azusa.
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
