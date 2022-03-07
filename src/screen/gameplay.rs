use super::{result::ResultScreen, select::SelectScreen, GameData, Screen};
use crate::{azusa::ClientPacket, chart::Chart, score::ScoreRecorder};
use async_trait::async_trait;
use kira::instance::{
    InstanceSettings, PauseInstanceSettings, ResumeInstanceSettings, StopInstanceSettings,
};
use macroquad::prelude::*;
use num_format::{Locale, ToFormattedString};
use std::{ops::Add, sync::Arc};

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

pub struct Gameplay {
    recorder: ScoreRecorder,

    time: f32,
    predicted_time: f32,
    prev_time: f32,
    position: f32,
    hyper_multiplier: f32,

    show_debug_hitbox: bool,

    chart: Chart,
    queued_fruits: Vec<usize>,
    deref_delete: Vec<usize>,

    time_countdown: f32,
    started: bool,
}

impl Gameplay {
    pub async fn new(data: Arc<GameData>, chart_name: &str, diff: &str) -> Self {
        let beatmap_data = load_file(&format!("resources/{}/{}.osu", chart_name, diff))
            .await
            .unwrap();
        let beatmap_content = std::str::from_utf8(&beatmap_data).unwrap();
        let beatmap =
            osu_parser::load_content(beatmap_content, osu_parser::BeatmapParseOptions::default())
                .unwrap();
        let chart = Chart::from_beatmap(&beatmap);

        let mut sound = data
            .audio_cache
            .get_sound(
                &mut *data.audio.lock(),
                &format!("resources/{}/audio.wav", chart_name),
            )
            .await;

        data.state
            .lock()
            .music
            .stop(StopInstanceSettings::new())
            .unwrap();
        data.state.lock().music = sound.play(InstanceSettings::default().volume(0.5)).unwrap();
        data.state
            .lock()
            .music
            .pause(PauseInstanceSettings::new())
            .unwrap();

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
            time_countdown,
            started: false,
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
    async fn update(&mut self, data: Arc<GameData>) {
        let binds = data.state.lock().binds;
        let catcher_y = self.catcher_y();

        if !self.started {
            self.prev_time = self.time;
            self.time = -self.time_countdown;
            if self.time_countdown > 0. {
                self.time_countdown -= get_frame_time();
            } else {
                data.state
                    .lock()
                    .music
                    .resume(ResumeInstanceSettings::new())
                    .unwrap();
                self.started = true;
            }
        } else {
            self.prev_time = self.time;
            self.time = data.state.lock().music.position() as f32;
        }
        if self.time - self.prev_time == 0. {
            let audio_frame_skip = data.state.lock().audio_frame_skip;
            if audio_frame_skip > 0 {
                self.predicted_time += get_frame_time();
            }
        } else {
            // Print prediction error
            /*let audio_frame_skip = data.state.lock().audio_frame_skip;
            let prediction_delta = self.time - self.predicted_time;
            if audio_frame_skip != 0 {
                let audio_frame_time = get_frame_time() * audio_frame_skip as f32;
                let prediction_off = prediction_delta / audio_frame_time;
                info!("Off by {:.2}%", prediction_off);
            }
            if prediction_delta < 0. {
                info!(
                    "Overcompensated by {}ms",
                    (-prediction_delta * 1000.).round() as i32
                );
            }*/
            self.predicted_time = self.time;
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
                self.recorder.register_judgement(true);
                self.deref_delete.push(idx);
            }
            if miss {
                self.recorder.register_judgement(false);
                self.deref_delete.push(idx);
            }
            if hit || miss {
                self.hyper_multiplier = fruit.hyper.unwrap_or(1.);
            }
        }

        for idx in self.deref_delete.drain(..).rev() {
            self.queued_fruits.remove(idx);
        }

        if self.recorder.hp == 0. || self.queued_fruits.is_empty() {
            let diff_idx = data.state.lock().difficulty_idx;
            let diff_id = data.state.lock().chart.difficulties[diff_idx].id;
            let score = self.recorder.to_score(diff_id);
            data.state.lock().leaderboard.submit_score(&score).await;
            data.state.lock().queued_screen = Some(Box::new(ResultScreen::new(&score)));
            data.packet_chan.send(ClientPacket::Submit(score)).unwrap();
        }

        if is_key_pressed(KeyCode::O) {
            self.show_debug_hitbox = !self.show_debug_hitbox;
        }
        if is_key_pressed(KeyCode::Escape) {
            data.state
                .lock()
                .music
                .stop(StopInstanceSettings::new())
                .unwrap();
            data.state.lock().queued_screen = Some(Box::new(SelectScreen::new(data.clone())));
        }
    }

    fn draw(&self, data: Arc<GameData>) {
        if let Some(background) = data.state.lock().background {
            draw_texture_ex(
                background,
                0.,
                0.,
                Color::new(1., 1., 1., 0.2),
                DrawTextureParams {
                    dest_size: Some(vec2(screen_width(), screen_height())),
                    ..Default::default()
                },
            );
        }
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
            let y = self.fruit_y(self.predicted_time, fruit.time);
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

        let text = format!("{}%", self.recorder.hp * 100.);
        let text_dim = measure_text(&text, None, 36, 1.0);
        draw_text(
            &text,
            screen_width() / 2. - text_dim.width / 2.,
            23.,
            36.,
            WHITE,
        );
    }
}
