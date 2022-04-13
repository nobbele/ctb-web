use async_trait::async_trait;
use macroquad::prelude::*;

use crate::draw_text_centered;

use super::{
    game::{GameMessage, SharedGameData},
    select::SelectScreen,
    Screen,
};

pub struct Visualizer {}

impl Visualizer {
    pub fn new() -> Self {
        Visualizer {}
    }
}

#[async_trait(?Send)]
impl Screen for Visualizer {
    async fn update(&mut self, data: SharedGameData) {
        if is_key_pressed(KeyCode::Escape) {
            data.broadcast(GameMessage::change_screen(SelectScreen::new(data.clone())));
        }
    }

    fn draw(&self, data: SharedGameData) {
        let receptor_y = screen_height() * 2. / 3.;

        let text = format!(
            "Audio Frame Skips: {}",
            data.state.borrow().audio_frame_skip
        );
        let text_measurements = measure_text(&text, None, 32, 1.);
        draw_text_centered(
            &text,
            screen_width() / 2.,
            text_measurements.height + text_measurements.offset_y,
            32,
            WHITE,
        );
        draw_rectangle(0., receptor_y, screen_width(), 2., PURPLE);

        let x = screen_width() / 3. - screen_width() / 20.;
        draw_text("Real Time", x, receptor_y - 10., 32.0, WHITE);
        draw_with_time(x, data.time(), receptor_y);

        let x = screen_width() * 2. / 3. - screen_width() / 20.;
        draw_text("Predicted Time", x, receptor_y - 10., 32.0, WHITE);
        draw_with_time(x, data.predicted_time(), receptor_y);
    }
}

fn draw_with_time(x: f32, music_time: f32, receptor_y: f32) {
    for time in next_n((music_time - 3.0).max(0.0), 10) {
        let y = calc_y(music_time, time, 2., receptor_y);
        draw_rectangle(
            x,
            y,
            screen_width() / 10.,
            10.,
            if (music_time - time).abs() <= 0.1 {
                RED
            } else {
                WHITE
            },
        );
    }
}

fn next_n(time: f32, count: u32) -> impl Iterator<Item = f32> {
    let time_sec = time.trunc() as u32;
    (time_sec..time_sec + count).map(|s| s as f32)
}

pub fn calc_y(time: f32, target: f32, fall_time: f32, height: f32) -> f32 {
    let time_left = target - time;
    let progress = 1. - (time_left / fall_time);
    height * progress
}
