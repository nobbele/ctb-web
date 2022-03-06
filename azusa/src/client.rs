use ctb_web::{
    azusa::{ClientPacket, ServerPacket},
    chat::{ChatMessage, ChatMessagePacket},
};
use std::time::Instant;

use crate::app::Target;

pub struct Client {
    tx: flume::Sender<(Target, ServerPacket)>,
    last_ping: Instant,
    username: String,
}

impl Client {
    pub fn new(username: String, tx: flume::Sender<(Target, ServerPacket)>) -> Self {
        tx.send((Target::User(username.clone()), ServerPacket::Connected))
            .unwrap();
        tx.send((
            Target::User(username.clone()),
            ServerPacket::Chat(ChatMessagePacket(ChatMessage {
                username: "Azusa".to_owned(),
                content: "Welcome to ctb-web!".to_owned(),
            })),
        ))
        .unwrap();

        Client {
            tx,
            username,
            last_ping: Instant::now(),
        }
    }
}

impl Client {
    pub fn handle(&mut self, packet: ClientPacket) {
        match packet {
            ClientPacket::Echo(s) => {
                self.tx
                    .send((Target::User(self.username.clone()), ServerPacket::Echo(s)))
                    .unwrap();
            }
            ClientPacket::Ping => {
                self.tx
                    .send((Target::User(self.username.clone()), ServerPacket::Pong))
                    .unwrap();
                self.last_ping = Instant::now();
                self.tx
                    .send((
                        Target::User(self.username.clone()),
                        ServerPacket::Chat(ChatMessagePacket(ChatMessage {
                            username: "Azusa".to_owned(),
                            content: "Ping-Pong".to_owned(),
                        })),
                    ))
                    .unwrap();
            }
            ClientPacket::Chat(content) => {
                self.tx
                    .send((
                        Target::Everyone,
                        ServerPacket::Chat(ChatMessagePacket(ChatMessage {
                            username: self.username.clone(),
                            content: content.clone(),
                        })),
                    ))
                    .unwrap();
            }
            ClientPacket::Login => panic!("Can't login after already being logged in!"),
        }
    }
}
