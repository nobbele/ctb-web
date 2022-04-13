use super::Overlay;
use crate::screen::game::SharedGameData;
use async_trait::async_trait;
use macroquad::prelude::*;

pub struct Settings {}

impl Settings {
    pub fn new() -> Self {
        Settings {}
    }
}

#[async_trait(?Send)]
impl Overlay for Settings {
    async fn update(&mut self, _data: SharedGameData) {}

    fn draw(&self, _data: SharedGameData) {
        draw_rectangle(
            screen_width() * 0.1,
            screen_height() * 0.1,
            screen_width() * 0.8,
            screen_height() * 0.8,
            Color::from_rgba(64, 64, 64, 238),
        );
    }
}
