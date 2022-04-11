use super::{game::GameMessage, select::SelectScreen, GameData, Screen};
use crate::{draw_text_centered, score::Score};
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
    pub passed: bool,
}

impl ResultScreen {
    pub fn new(score: &Score, title: String, difficulty: String) -> Self {
        ResultScreen {
            title,
            difficulty,
            score: score.score,
            hit_count: score.hit_count,
            miss_count: score.miss_count,
            top_combo: score.top_combo,
            accuracy: score.hit_count as f32 / (score.hit_count + score.miss_count) as f32,
            passed: score.passed,
        }
    }
}

#[async_trait(?Send)]
impl Screen for ResultScreen {
    fn draw(&self, data: Arc<GameData>) {
        if let Some(background) = data.state.lock().background {
            draw_texture_ex(
                background,
                0.,
                0.,
                Color::new(0.5, 0.5, 0.5, 0.2),
                DrawTextureParams {
                    dest_size: Some(vec2(screen_width(), screen_height())),
                    ..Default::default()
                },
            );
        }

        draw_text_centered(
            &format!(
                "{} [{}] ({})",
                self.title,
                self.difficulty,
                if self.passed { "Passed" } else { "Failed" }
            ),
            screen_width() / 2.,
            screen_height() / 2. - 100.,
            36,
            WHITE,
        );
        draw_text_centered(
            &format!("{}x", self.top_combo),
            screen_width() / 2.,
            screen_height() / 2. - 10.,
            36,
            WHITE,
        );
        draw_text_centered(
            &format!("{}/{}", self.hit_count, self.miss_count),
            screen_width() / 2.,
            screen_height() / 2. + 30.,
            36,
            WHITE,
        );
        draw_text_centered(
            &format!("{}", self.score),
            screen_width() / 2.,
            screen_height() / 2. + 70.,
            36,
            WHITE,
        );
        draw_text_centered(
            &format!("{:.2}%", self.accuracy * 100.),
            screen_width() / 2.,
            screen_height() / 2. + 110.,
            36,
            WHITE,
        );
    }

    async fn update(&mut self, data: Arc<GameData>) {
        if is_key_pressed(KeyCode::Escape) {
            data.broadcast(GameMessage::change_screen(SelectScreen::new(data.clone())));
        }
    }
}
