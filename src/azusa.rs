use std::sync::Arc;

use flume::Receiver;
use message_io::{
    network::{NetEvent, Transport},
    node::{self, NodeEvent, NodeHandler, NodeTask},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Echo(String),
    Connected,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Echo(String),
}

pub struct Azusa {
    _task: NodeTask,
    receiver: Receiver<ServerMessage>,
    handler: Arc<NodeHandler<ClientMessage>>,
}

impl Azusa {
    pub fn new() -> Self {
        let (handler, listener) = node::split();
        let handler = Arc::new(handler);
        let (server, _) = handler
            .network()
            .connect(Transport::FramedTcp, "127.0.0.1:3042")
            .unwrap();

        let (sender, receiver) = flume::unbounded();

        let task = listener.for_each_async({
            let handler = handler.clone();
            move |event| match event {
                NodeEvent::Network(net_event) => match net_event {
                    NetEvent::Connected(_endpoint, _ok) => (),
                    NetEvent::Accepted(_, _) => unreachable!(),
                    NetEvent::Message(_endpoint, data) => {
                        let msg: ServerMessage = bincode::deserialize(data).unwrap();
                        println!("Received: {:?}", msg);
                        sender.send(msg).unwrap();
                    }
                    NetEvent::Disconnected(_endpoint) => (),
                },
                NodeEvent::Signal(msg) => {
                    handler
                        .network()
                        .send(server, &bincode::serialize(&msg).unwrap());
                }
            }
        });

        Azusa {
            _task: task,
            receiver,
            handler,
        }
    }

    pub fn receive(&self) -> impl Iterator<Item = ServerMessage> + '_ {
        self.receiver.drain()
    }

    pub fn send(&self, message: ClientMessage) {
        self.handler.signals().send(message);
    }
}
