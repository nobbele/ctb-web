use super::{select::SelectScreen, GameData, Screen};
use crate::score::Score;
use async_trait::async_trait;
use macroquad::prelude::*;
use std::sync::Arc;

pub struct ResultScreen {
    pub title: String,
    pub difficulty: String,

    pub score: u32,
    pub hit_count: u32,
    pub miss_count: u32,
    pub top_combo: u32,
    pub accuracy: f32,
}

impl ResultScreen {
    pub fn new(score: &Score) -> Self {
        ResultScreen {
            title: "TODO".to_string(),
            difficulty: "TODO".to_string(),
            score: score.score,
            hit_count: score.hit_count,
            miss_count: score.miss_count,
            top_combo: score.top_combo,
            accuracy: score.hit_count as f32 / (score.hit_count + score.miss_count) as f32,
        }
    }
}

#[async_trait(?Send)]
impl Screen for ResultScreen {
    fn draw(&self, _data: Arc<GameData>) {
        draw_text(
            &self.title,
            screen_width() / 2.,
            screen_height() / 2. - 100.,
            36.,
            WHITE,
        );
        draw_text(
            &self.difficulty,
            screen_width() / 2.,
            screen_height() / 2. - 50.,
            36.,
            WHITE,
        );
        draw_text(
            &format!("{}x", self.top_combo),
            screen_width() / 2.,
            screen_height() / 2. - 10.,
            36.,
            WHITE,
        );
        draw_text(
            &format!("{}/{}", self.hit_count, self.miss_count),
            screen_width() / 2.,
            screen_height() / 2. + 30.,
            36.,
            WHITE,
        );
        draw_text(
            &format!("{}", self.score),
            screen_width() / 2.,
            screen_height() / 2. + 70.,
            36.,
            WHITE,
        );
        draw_text(
            &format!("{:.2}%", self.accuracy * 100.),
            screen_width() / 2.,
            screen_height() / 2. + 110.,
            36.,
            WHITE,
        );
    }

    async fn update(&mut self, data: Arc<GameData>) {
        if is_key_pressed(KeyCode::Escape) {
            data.state.lock().queued_screen = Some(Box::new(SelectScreen::new(data.clone())));
        }
    }
}
