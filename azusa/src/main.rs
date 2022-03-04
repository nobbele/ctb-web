use std::{
    sync::{Arc, Mutex},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use ctb_web::{
    azusa::{ClientPacket, ServerPacket},
    chat::{ChatMessage, ChatMessagePacket},
};
use ws::listen;

trait WsExt {
    fn send_packet(&self, packet: &ServerPacket) -> ws::Result<()>;
}

impl WsExt for ws::Sender {
    fn send_packet(&self, packet: &ServerPacket) -> ws::Result<()> {
        self.send(bincode::serialize(packet).unwrap().as_slice())
    }
}

struct Client {
    origin: ws::Sender,
    last_ping: Instant,
    username: String,
    senders: Arc<Mutex<Vec<ws::Sender>>>,
}

impl Client {
    pub fn new(origin: ws::Sender, senders: Arc<Mutex<Vec<ws::Sender>>>) -> Self {
        Client {
            username: format!(
                "{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            origin,
            last_ping: Instant::now(),
            senders,
        }
    }
}

impl ws::Handler for Client {
    fn on_open(&mut self, _shake: ws::Handshake) -> ws::Result<()> {
        self.origin.send_packet(&ServerPacket::Connected)?;
        self.origin
            .send_packet(&ServerPacket::Chat(ChatMessagePacket(ChatMessage {
                username: "Azusa".to_owned(),
                content: "Welcome to ctb-web!".to_owned(),
            })))?;
        Ok(())
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        let packet: ClientPacket = bincode::deserialize(msg.into_data().as_slice())?;
        println!("Server got packet '{:?}'", packet);

        match packet {
            ClientPacket::Echo(s) => {
                self.origin.send_packet(&ServerPacket::Echo(s))?;
            }
            ClientPacket::Ping => {
                self.origin.send_packet(&ServerPacket::Pong)?;
                self.last_ping = Instant::now();
                self.origin
                    .send_packet(&ServerPacket::Chat(ChatMessagePacket(ChatMessage {
                        username: "Azusa".to_owned(),
                        content: "Ping-Pong".to_owned(),
                    })))?;
            }
            ClientPacket::Chat(content) => {
                for sender in self.senders.lock().unwrap().iter() {
                    sender.send_packet(&ServerPacket::Chat(ChatMessagePacket(ChatMessage {
                        username: self.username.clone(),
                        content: content.clone(),
                    })))?;
                }
            }
        }
        Ok(())
    }
}

fn main() {
    let messages = Arc::new(Mutex::new(Vec::new()));
    listen("127.0.0.1:3012", |out| {
        println!("New client!");
        messages.lock().unwrap().push(out.clone());
        Client::new(out, messages.clone())
    })
    .expect("Unable to open server");
}
