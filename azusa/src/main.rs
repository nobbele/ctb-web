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

fn main() {
    listen("127.0.0.1:3012", |out| {
        println!("New client!");
        out.send_packet(&ServerPacket::Connected).unwrap();
        move |msg: ws::Message| {
            let packet: ClientPacket = bincode::deserialize(msg.into_data().as_slice())?;
            println!("Server got packet '{:?}'", packet);

            //out.send(msg)
            Ok(())
        }
    })
    .expect("Unable to open server");
}
