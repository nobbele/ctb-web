use super::Overlay;
use crate::{azusa::ClientPacket, screen::game::SharedGameData};
use async_trait::async_trait;
use macroquad::prelude::*;
use std::fmt::Write;

pub struct ChatOverlay {
    text_buffer: String,
}

impl ChatOverlay {
    pub fn new() -> Self {
        ChatOverlay {
            text_buffer: String::new(),
        }
    }
}

#[async_trait(?Send)]
impl Overlay for ChatOverlay {
    async fn update(&mut self, data: SharedGameData) {
        while let Some(char) = get_char_pressed() {
            self.text_buffer.write_char(char).unwrap();
        }

        if is_key_pressed(KeyCode::Enter) {
            let message = std::mem::replace(&mut self.text_buffer, String::new());
            data.send_server(ClientPacket::Chat(message));
        }
    }

    fn draw(&self, data: SharedGameData) {
        draw_rectangle(
            5.0,
            screen_height() - 205.0,
            screen_width() - 10.0,
            200.0,
            Color::from_rgba(64, 64, 64, 192),
        );

        let state = data.state();
        let messages = state.chat.messages();
        for (i, message) in messages.iter().enumerate() {
            draw_text(
                &format!("{}: {}", message.username, message.content),
                15.0,
                screen_height() - 205.0 + (i + 1) as f32 * 18.0,
                16.0,
                Color::from_rgba(255, 255, 255, 255),
            );
        }

        draw_text(
            &self.text_buffer,
            12.0,
            screen_height() - 15.0,
            24.0,
            Color::from_rgba(255, 240, 240, 255),
        )
    }
}
