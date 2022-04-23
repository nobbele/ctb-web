use super::{
    game::{GameMessage, SharedGameData},
    gameplay::Replay,
    select::SelectScreen,
    Screen,
};
use crate::{
    draw_text_centered,
    rulesets::Ruleset,
    score::{self, Score},
};
use async_trait::async_trait;
use instant::SystemTime;
use macroquad::prelude::*;
use serde::Serialize;

pub struct ResultScreen<R: Ruleset> {
    title: String,
    difficulty: String,

    score: Score<R::Judgement>,
    replay: Replay<R::Input, R::SyncFrame>,
}

impl<R: Ruleset> ResultScreen<R> {
    pub fn new(
        score: Score<R::Judgement>,
        replay: Replay<R::Input, R::SyncFrame>,
        title: String,
        difficulty: String,
    ) -> Self {
        ResultScreen {
            title,
            difficulty,
            score,
            replay,
        }
    }
}

#[async_trait(?Send)]
impl<R: Ruleset> Screen for ResultScreen<R> {
    async fn update(&mut self, data: SharedGameData) {
        if is_key_pressed(KeyCode::Escape) {
            data.broadcast(GameMessage::change_screen(SelectScreen::new(data.clone())));
        }

        if is_key_pressed(KeyCode::F2) {
            let date_time = time::OffsetDateTime::from_unix_timestamp(
                self.replay
                    .start
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            )
            .unwrap();
            let replay_name = format!(
                    "{} - {} ({}).crp",
                    self.title,
                    self.difficulty,
                    date_time
                        .format(
                            &time::format_description::parse(
                                "[year repr:full] [month repr:short padding:zero] [day] [hour repr:24 padding:zero]:[minute padding:zero]"
                            )
                            .unwrap()
                        )
                        .unwrap()
                );
            let mut ser = rmp_serde::Serializer::new(
                std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&replay_name)
                    .unwrap(),
            )
            .with_binary();
            self.replay.serialize(&mut ser).unwrap();
        }
    }

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
}
