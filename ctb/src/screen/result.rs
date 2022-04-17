use super::{
    game::{GameMessage, SharedGameData},
    select::SelectScreen,
    Screen,
};
use crate::{
    draw_text_centered,
    score::{self, Judgement, Score},
};
use async_trait::async_trait;
use macroquad::prelude::*;

pub struct ResultScreen<J: Judgement> {
    title: String,
    difficulty: String,

    score: Score<J>,
}

impl<J: Judgement> ResultScreen<J> {
    pub fn new(score: Score<J>, title: String, difficulty: String) -> Self {
        ResultScreen {
            title,
            difficulty,
            score,
        }
    }
}

#[async_trait(?Send)]
impl<J: Judgement> Screen for ResultScreen<J> {
    fn draw(&self, data: SharedGameData) {
        draw_texture_ex(
            data.background(),
            0.,
            0.,
            Color::new(0.5, 0.5, 0.5, 0.2),
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height())),
                ..Default::default()
            },
        );

        draw_text_centered(
            &format!(
                "{} [{}] ({})",
                self.title,
                self.difficulty,
                if self.score.passed {
                    "Passed"
                } else {
                    "Failed"
                }
            ),
            screen_width() / 2.,
            screen_height() / 2. - 100.,
            36,
            WHITE,
        );
        draw_text_centered(
            &format!("{}x", self.score.top_combo),
            screen_width() / 2.,
            screen_height() / 2. - 10.,
            36,
            WHITE,
        );
        draw_text_centered(
            &self
                .score
                .judgements
                .iter()
                .map(|(_, &count)| count.to_string())
                .collect::<Vec<String>>()
                .join("/"),
            screen_width() / 2.,
            screen_height() / 2. + 30.,
            36,
            WHITE,
        );
        draw_text_centered(
            &format!("{}", self.score.score),
            screen_width() / 2.,
            screen_height() / 2. + 70.,
            36,
            WHITE,
        );
        draw_text_centered(
            &format!("{:.2}%", score::accuracy(&self.score.judgements) * 100.),
            screen_width() / 2.,
            screen_height() / 2. + 110.,
            36,
            WHITE,
        );
    }

    async fn update(&mut self, data: SharedGameData) {
        if is_key_pressed(KeyCode::Escape) {
            data.broadcast(GameMessage::change_screen(SelectScreen::new(data.clone())));
        }
    }
}
