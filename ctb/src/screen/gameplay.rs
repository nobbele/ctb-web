use super::{
    game::{GameMessage, SharedGameData},
    result::ResultScreen,
    select::SelectScreen,
    Screen,
};
use crate::{
    azusa::ClientPacket,
    chart::Chart,
    draw_text_centered, math,
    score::{Judgement, Score, ScoreRecorder},
};
use async_trait::async_trait;
use kira::instance::ResumeInstanceSettings;
use macroquad::prelude::*;
use num_format::{Locale, ToFormattedString};
use std::ops::Add;

pub fn can_catch_fruit(catcher_hitbox: Rect, fruit_hitbox: Rect) -> bool {
    catcher_hitbox.intersect(fruit_hitbox).is_some()
}

pub fn catcher_speed(dashing: bool, hyper_multiplier: f32) -> f32 {
    let mut mov_speed = 500.;
    if dashing {
        mov_speed *= 2. * hyper_multiplier;
    }
    mov_speed
}

#[derive(
    Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize, Clone, PartialOrd, Ord,
)]
pub enum CatchJudgement {
    Perfect,
    Miss,
}

impl Judgement for CatchJudgement {
    fn hit(_inaccuracy: f32) -> Self {
        Self::Perfect
    }

    fn miss() -> Self {
        Self::Miss
    }

    fn weight(&self) -> f32 {
        match self {
            CatchJudgement::Perfect => 1.0,
            CatchJudgement::Miss => 0.0,
        }
    }

    fn all() -> Vec<Self> {
        vec![CatchJudgement::Perfect, CatchJudgement::Miss]
    }
}

pub type CatchScoreRecorder = ScoreRecorder<CatchJudgement>;
pub type CatchScore = Score<CatchJudgement>;

pub struct Gameplay {
    recorder: ScoreRecorder<CatchJudgement>,

    time: f32,
    predicted_time: f32,

    prev_time: f32,
    position: f32,
    hyper_multiplier: f32,

    show_debug_hitbox: bool,
    use_predicted_time: bool,

    chart: Chart,
    queued_fruits: Vec<usize>,
    deref_delete: Vec<usize>,

    time_countdown: f32,
    fade_out: f32,
    started: bool,
    ended: bool,

    paused: bool,
}

impl Gameplay {
    pub async fn new(data: SharedGameData, chart_name: &str, diff: &str) -> Self {
        let beatmap_data = load_file(&format!("resources/{}/{}.osu", chart_name, diff))
            .await
            .unwrap();
        let beatmap_content = std::str::from_utf8(&beatmap_data).unwrap();
        let beatmap =
            osu_parser::load_content(beatmap_content, osu_parser::BeatmapParseOptions::default())
                .unwrap();
        let chart = Chart::from_beatmap(&beatmap);

        let sound = data
            .audio_cache
            .get_sound(
                &mut *data.audio.borrow_mut(),
                &format!("resources/{}/audio.wav", chart_name),
            )
            .await;

        // Time from the last fruit to the end of the music.
        let time_to_end = sound.duration() as f32 - chart.fruits.last().unwrap().time;

        data.broadcast(GameMessage::update_music(sound));
        data.broadcast(GameMessage::PauseMusic);

        let first_fruit = chart.fruits.first().unwrap();
        let min_time_required = first_fruit.position / catcher_speed(false, 1.0);
        let time_countdown = if min_time_required * 0.5 > first_fruit.time {
            1.
        } else {
            0.
        };
        next_frame().await;

        Gameplay {
            time: -time_countdown,
            predicted_time: -time_countdown,
            prev_time: -time_countdown,
            recorder: ScoreRecorder::new(chart.fruits.len() as u32),
            position: 256.,
            hyper_multiplier: 1.,
            deref_delete: Vec::new(),
            queued_fruits: (0..chart.fruits.len()).collect(),
            chart,
            show_debug_hitbox: false,
            use_predicted_time: true,
            time_countdown,
            started: false,
            fade_out: time_to_end.max(1.).min(3.),
            ended: false,
            paused: false,
        }
    }

    pub fn catcher_y(&self) -> f32 {
        screen_height() - 148.
    }

    pub fn fruit_y(&self, time: f32, target: f32) -> f32 {
        let time_left = target - time;
        let progress = 1. - (time_left / self.chart.fall_time);
        self.catcher_y() * progress
    }

    pub fn catcher_speed(&self, dash: KeyCode) -> f32 {
        catcher_speed(is_key_down(dash), self.hyper_multiplier)
    }

    pub fn playfield_to_screen_x(&self, x: f32) -> f32 {
        let visual_width = self.playfield_width() * self.scale();
        let playfield_x = screen_width() / 2. - visual_width / 2.;
        playfield_x + x * self.scale()
    }

    pub fn scale(&self) -> f32 {
        let scale = screen_width() / 512.;
        scale * 2. / 3.
    }

    pub fn playfield_width(&self) -> f32 {
        512.
    }

    pub fn movement_direction(&self, left: KeyCode, right: KeyCode) -> i32 {
        is_key_down(right) as i32 - is_key_down(left) as i32
    }
}

#[async_trait(?Send)]
impl Screen for Gameplay {
    async fn update(&mut self, data: SharedGameData) {
        let binds = data.state().binds;
        let catcher_y = self.catcher_y();

        if !self.started {
            self.prev_time = self.time;
            self.time = -self.time_countdown;
            self.predicted_time = -self.time_countdown;
            if self.time_countdown > 0. {
                self.time_countdown -= get_frame_time();
            } else {
                data.state_mut()
                    .music
                    .resume(ResumeInstanceSettings::new())
                    .unwrap();
                self.started = true;
            }
        } else {
            self.prev_time = self.time;
            self.time = data.time();
            self.predicted_time = data.predicted_time();
        }

        self.position = self
            .position
            .add(
                self.movement_direction(binds.left, binds.right) as f32
                    * self.catcher_speed(binds.dash)
                    * get_frame_time(),
            )
            .clamp(0., self.playfield_width());

        let fruit_travel_distance = self.fruit_y(self.time, 0.) - self.fruit_y(self.prev_time, 0.);

        let catcher_hitbox = Rect::new(
            self.position - self.chart.catcher_width / 2.,
            catcher_y - fruit_travel_distance / 2.,
            self.chart.catcher_width,
            fruit_travel_distance,
        );

        for (idx, fruit) in self.queued_fruits.iter().enumerate() {
            let fruit = self.chart.fruits[*fruit];
            // Last frame hitbox.
            let fruit_hitbox = Rect::new(
                fruit.position - self.chart.fruit_radius,
                self.fruit_y(self.prev_time, fruit.time) - fruit_travel_distance / 2.,
                self.chart.fruit_radius * 2.,
                fruit_travel_distance,
            );
            let hit = can_catch_fruit(catcher_hitbox, fruit_hitbox);
            let miss = fruit_hitbox.y >= screen_height();
            assert!(!(hit && miss), "Can't hit and miss at the same time!");

            if hit {
                let player_position_panning = math::remap(
                    0.,
                    self.playfield_width(),
                    data.panning().0,
                    data.panning().1,
                    self.position,
                );

                self.recorder.register_judgement(CatchJudgement::Perfect);
                self.deref_delete.push(idx);

                self.hyper_multiplier = fruit.hyper.unwrap_or(1.);

                if !fruit.small {
                    data.hit_normal
                        .borrow_mut()
                        .play(
                            kira::instance::InstanceSettings::default()
                                .volume(data.total_hitsound_volume() as f64)
                                .panning(player_position_panning as f64),
                        )
                        .unwrap();
                }
            }
            if miss {
                if self.recorder.combo >= 8 {
                    data.combo_break
                        .borrow_mut()
                        .play(
                            kira::instance::InstanceSettings::new()
                                .volume(data.total_hitsound_volume() as f64),
                        )
                        .unwrap();
                }

                self.recorder.register_judgement(CatchJudgement::Miss);
                self.deref_delete.push(idx);
            }
        }

        for idx in self.deref_delete.drain(..).rev() {
            self.queued_fruits.remove(idx);
        }

        if self.queued_fruits.is_empty() && !self.ended {
            self.fade_out -= get_frame_time();

            if self.fade_out <= 0. {
                let diff_id = data.state().difficulty().id;
                let score = self.recorder.to_score(diff_id);
                if score.passed {
                    data.state_mut().leaderboard.submit_score(&score).await;
                }

                let map_title = data.state().chart.title.clone();
                let diff_title = data.state().difficulty().name.clone();
                data.send_server(ClientPacket::Submit(score.clone()));
                data.broadcast(GameMessage::change_screen(ResultScreen::new(
                    score, map_title, diff_title,
                )));

                self.ended = true;
            }
        }

        if is_key_pressed(KeyCode::O) {
            self.show_debug_hitbox = !self.show_debug_hitbox;
        }
        if is_key_pressed(KeyCode::P) {
            self.use_predicted_time = !self.use_predicted_time;
        }
        if is_key_pressed(KeyCode::End) {
            data.broadcast(GameMessage::change_screen(SelectScreen::new(data.clone())));
        }
        if is_key_pressed(KeyCode::Escape) {
            self.paused = !self.paused;

            if self.paused {
                data.broadcast(GameMessage::PauseMusic);
            } else {
                data.broadcast(GameMessage::ResumeMusic);
            }
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
        draw_line(
            self.playfield_to_screen_x(0.) + 2. / 2.,
            0.,
            self.playfield_to_screen_x(0.) + 2. / 2.,
            screen_height(),
            2.,
            RED,
        );

        draw_line(
            self.playfield_to_screen_x(self.playfield_width()) + 2. / 2.,
            0.,
            self.playfield_to_screen_x(self.playfield_width()) + 2. / 2.,
            screen_height(),
            2.,
            RED,
        );

        let fruit_travel_distance = self.fruit_y(self.time, 0.) - self.fruit_y(self.prev_time, 0.);
        let catcher_hitbox = Rect::new(
            self.position - self.chart.catcher_width / 2.,
            self.catcher_y() - fruit_travel_distance / 2.,
            self.chart.catcher_width,
            fruit_travel_distance,
        );

        for fruit in &self.queued_fruits {
            let fruit = self.chart.fruits[*fruit];
            let y = self.fruit_y(
                if self.use_predicted_time {
                    self.predicted_time
                } else {
                    self.time
                },
                fruit.time,
            );
            if y + self.chart.fruit_radius * self.scale() <= 0. {
                // queued_fruits are in spawn/hit order currently.
                // I may change it in the future.
                // but for now this exists to improve performance.
                //break;
            }

            let mut radius = self.chart.fruit_radius * self.scale();
            if fruit.small {
                radius /= 2.0;
            }

            draw_texture_ex(
                data.fruit,
                self.playfield_to_screen_x(fruit.position) - radius,
                y - radius,
                if fruit.hyper.is_some() { RED } else { WHITE },
                DrawTextureParams {
                    dest_size: Some(vec2(radius * 2., radius * 2.)),
                    ..Default::default()
                },
            );
            if self.show_debug_hitbox {
                let fruit_hitbox = Rect::new(
                    fruit.position - self.chart.fruit_radius,
                    self.fruit_y(self.time, fruit.time) - fruit_travel_distance / 2.,
                    self.chart.fruit_radius * 2.,
                    fruit_travel_distance,
                );
                let prev_fruit_hitbox = Rect::new(
                    fruit.position - self.chart.fruit_radius,
                    self.fruit_y(self.prev_time, fruit.time) - fruit_travel_distance / 2.,
                    self.chart.fruit_radius * 2.,
                    fruit_travel_distance,
                );

                draw_rectangle(
                    self.playfield_to_screen_x(fruit_hitbox.x),
                    fruit_hitbox.y,
                    fruit_hitbox.w * self.scale(),
                    fruit_hitbox.h,
                    BLUE,
                );
                draw_rectangle(
                    self.playfield_to_screen_x(prev_fruit_hitbox.x),
                    prev_fruit_hitbox.y,
                    prev_fruit_hitbox.w * self.scale(),
                    prev_fruit_hitbox.h,
                    GREEN,
                );
            }
        }
        if self.show_debug_hitbox {
            draw_rectangle(
                self.playfield_to_screen_x(catcher_hitbox.x),
                catcher_hitbox.y,
                catcher_hitbox.w * self.scale(),
                catcher_hitbox.h,
                RED,
            );
        }
        draw_texture_ex(
            data.catcher,
            self.playfield_to_screen_x(self.position)
                - self.chart.catcher_width * self.scale() / 2.,
            self.catcher_y(),
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(
                    self.chart.catcher_width * self.scale(),
                    self.chart.catcher_width * self.scale(),
                )),
                ..Default::default()
            },
        );

        draw_text(
            &format!("{:.2}%", self.recorder.accuracy * 100.),
            screen_width() - 116.,
            23.,
            36.,
            WHITE,
        );

        draw_text(
            &format!("{}x", self.recorder.combo),
            5.,
            screen_height() - 5.,
            36.,
            WHITE,
        );

        draw_text(
            &self.recorder.score.to_formatted_string(&Locale::en),
            5.,
            23.,
            36.,
            WHITE,
        );

        draw_text_centered(
            &format!("{}%", self.recorder.hp * 100.),
            screen_width() / 2.,
            23.0,
            36,
            WHITE,
        );
    }
}
