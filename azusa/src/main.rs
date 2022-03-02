use ctb_web::azusa::ClientMessage;
use message_io::network::{NetEvent, Transport};
use message_io::node;
use std::collections::HashMap;

struct Session {}

fn main() {
    let (handler, listener) = node::split();

    handler
        .network()
        .listen(Transport::FramedTcp, "0.0.0.0:3042")
        .unwrap();

    let mut sessions = HashMap::new();

    listener.for_each(move |event| match event {
        node::NodeEvent::Network(event) => match event {
            NetEvent::Connected(_, _) => unreachable!(),
            NetEvent::Accepted(endpoint, _listener) => {
                println!("Client connected");
                sessions.insert(endpoint, Session {});
                handler
                    .signals()
                    .send((endpoint, ctb_web::azusa::ServerMessage::Connected));
            }
            NetEvent::Message(endpoint, data) => {
                let msg: ClientMessage = bincode::deserialize(data).unwrap();
                println!("Received: {:?}", msg);
                match msg {
                    ClientMessage::Echo(s) => {
                        handler
                            .signals()
                            .send((endpoint, ctb_web::azusa::ServerMessage::Echo(s)));
                    }
                }
            }
            NetEvent::Disconnected(endpoint) => {
                println!("Client disconnected");
                sessions.remove(&endpoint);
            }
        },
        node::NodeEvent::Signal((endpoint, msg)) => {
            handler
                .network()
                .send(endpoint, &bincode::serialize(&msg).unwrap());
        }
    });
}
