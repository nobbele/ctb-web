use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessagePacket(pub ChatMessage);

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub username: String,
    pub content: String,
}

pub struct Chat {
    messages: Vec<ChatMessage>,
}

impl Chat {
    pub fn new() -> Self {
        Chat {
            messages: Vec::new(),
        }
    }

    pub fn handle_packet(&mut self, packet: ChatMessagePacket) {
        self.messages.push(packet.0);
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }
}
