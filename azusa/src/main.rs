use std::time::Instant;

use ctb_web::azusa::{ClientPacket, ServerPacket};
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
}

impl Client {
    pub fn new(origin: ws::Sender) -> Self {
        Client {
            origin,
            last_ping: Instant::now(),
        }
    }
}

impl ws::Handler for Client {
    fn on_open(&mut self, _shake: ws::Handshake) -> ws::Result<()> {
        self.origin.send_packet(&ServerPacket::Connected)?;
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
            }
        }
        Ok(())
    }
}

fn main() {
    listen("127.0.0.1:3012", |out| {
        println!("New client!");
        Client::new(out)
    })
    .expect("Unable to open server");
}
