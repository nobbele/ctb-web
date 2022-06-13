use super::{
    game::{GameMessage, SharedGameData},
    result::ResultScreen,
    select::SelectScreen,
    Screen,
};
use crate::{
    azusa::ClientPacket,
    chart::{Chart, EventData, HitSoundKind},
    convert::ConvertFrom,
    draw_text_centered,
    frozen::Frozen,
    math,
    rulesets::{
        catch::{catcher_speed, CatchInput, CatchRuleset},
        JudgementResult, Ruleset,
    },
    score::ScoreRecorder,
};
use async_trait::async_trait;
use instant::SystemTime;
use kira::tween::Tween;
use macroquad::{prelude::*, rand::rand};
use num_format::{Locale, ToFormattedString};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplaySyncFrame<F> {
    pub time: f32,
    pub data: F,
    pub input_index: u32,
}

mod system_time_serde {
    use instant::{Duration, SystemTime};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let timestamp = value
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        serializer.serialize_u64(timestamp)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let timestamp = u64::deserialize(deserializer)?;
        Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(timestamp))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Replay<I, S> {
    #[serde(with = "system_time_serde")]
    pub start: SystemTime,
    pub inputs: Vec<I>,
    pub sync_frames: Vec<ReplaySyncFrame<S>>,
}

impl<I, S> Replay<I, S> {
    pub fn new(predicted_frame_count: usize) -> Self {
        Replay {
            start: SystemTime::now(),
            inputs: Vec::with_capacity(predicted_frame_count),
            sync_frames: Vec::with_capacity(predicted_frame_count / 3),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReplayType {
    Record,
    Playback {
        input_index: usize,
        sync_frame_index: usize,
    },
}

pub struct DisposedFruit {
    gravity: f32,
    x_speed: f32,
    color: Color,
}

pub struct DisposedFruits {
    position: f32,
    fruits: Vec<DisposedFruit>,
    time_since_dispose: f32,
}

pub struct Gameplay<R: Ruleset> {
    chart_name: String,
    recorder: ScoreRecorder<R::Judgement>,
    replay: Replay<R::Input, R::SyncFrame>,
    replay_type: ReplayType,
    ruleset: R,

    time: f32,
    predicted_time: f32,

    prev_time: f32,
    show_debug_hitbox: bool,
    use_predicted_time: bool,

    chart: Frozen<Chart>,
    queued_fruits: Vec<usize>,
    plate: Vec<(f32, Color)>,
    disposed_fruits: Vec<DisposedFruits>,
    time_countdown: f32,
    fade_out: f32,
    started: bool,
    ended: bool,

    paused: bool,

    event_idx: usize,
    hitsound: HitSoundKind,
    bpm: f32,
    volume: f32,
}

impl Gameplay<CatchRuleset> {
    pub async fn new(data: SharedGameData, chart_name: &str, diff: &str) -> Self {
        let beatmap_data = load_file(&format!("resources/{}/{}.osu", chart_name, diff))
            .await
            .unwrap();
        let beatmap_content = std::str::from_utf8(&beatmap_data).unwrap();
        let beatmap =
            osu_parser::load_content(beatmap_content, osu_parser::BeatmapParseOptions::default())
                .unwrap();
        let chart = Chart::convert_from(&beatmap);

        let sound = data
            .audio_cache
            .get_sound(
                &format!("resources/{}/audio.wav", chart_name),
                data.main_track.id(),
            )
            .await
            .unwrap();

        // Time from the last fruit to the end of the music.
        let music_length = sound.duration().as_secs_f32();
        let time_to_end = music_length - chart.fruits.last().unwrap().time;

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

        // Assume 60 frames per second.
        let approx_frame_count = (music_length * 60.) as usize;

        let replay = Replay::new(approx_frame_count);

        Gameplay {
            chart_name: chart_name.to_owned(),
            ruleset: CatchRuleset::new(),

            replay,
            replay_type: ReplayType::Record,

            time: -time_countdown,
            predicted_time: -time_countdown,
            prev_time: -time_countdown,
            recorder: ScoreRecorder::new(chart.fruits.len() as u32),
            queued_fruits: (0..chart.fruits.len()).collect(),
            chart: Frozen(chart),
            show_debug_hitbox: false,
            use_predicted_time: true,
            time_countdown,
            started: false,
            fade_out: time_to_end.max(1.).min(3.),
            ended: false,
            paused: false,

            event_idx: 0,
            hitsound: HitSoundKind::Normal,
            bpm: 180.,
            volume: 1.,
            plate: vec![],
            disposed_fruits: vec![],
        }
    }

    fn catcher_y(&self) -> f32 {
        screen_height() - 148.
    }

    fn fruit_y(&self, time: f32, target: f32, fall_multiplier: f32) -> f32 {
        let time_left = target - time;
        let progress = 1. - (time_left / (self.chart.fall_time / fall_multiplier));
        self.catcher_y() * progress
    }

    fn playfield_to_screen_x(&self, x: f32, data: SharedGameData) -> f32 {
        let visual_width = self.playfield_width() * self.scale(data.clone());
        let playfield_x = screen_width() / 2. - visual_width / 2.;
        playfield_x + x * self.scale(data)
    }

    fn scale(&self, data: SharedGameData) -> f32 {
        let scale = screen_width() / self.playfield_width();
        scale * data.playfield_size.get()
    }

    fn playfield_width(&self) -> f32 {
        512.
    }

    fn drawable_fruit_color(&self, color: Color) -> Color {
        Color {
            r: math::lerp(color.r, 1., 0.75),
            g: math::lerp(color.g, 1., 0.75),
            b: math::lerp(color.b, 1., 0.75),
            a: math::lerp(color.a, 1., 0.75),
        }
    }

    fn dispose_plate(&mut self) {
        self.disposed_fruits.push(DisposedFruits {
            position: self.ruleset.position,
            fruits: self
                .plate
                .drain(..self.plate.len() - 1)
                .map(|(off, color)| DisposedFruit {
                    gravity: math::remap(0., u32::MAX as f32, 900., 1500., rand() as f32),
                    x_speed: off * 100.,
                    color,
                })
                .collect(),
            time_since_dispose: 0.,
        });
    }
}

#[async_trait(?Send)]
impl Screen for Gameplay<CatchRuleset> {
    async fn update(&mut self, data: SharedGameData) {
        let binds = data.state().binds;

        if !self.started {
            self.prev_time = self.time;
            self.time = -self.time_countdown;
            self.predicted_time = -self.time_countdown;
            if self.time_countdown > 0. {
                self.time_countdown -= get_frame_time();
            } else {
                data.broadcast(GameMessage::ResumeMusic);
                self.started = true;
            }
        } else {
            self.prev_time = self.time;
            self.time = data.time_with_offset();
            self.predicted_time = data.predicted_time_with_offset();
        }

        let mut should_dispose_plate = false;

        if !self.paused {
            for disposed_fruits in &mut self.disposed_fruits {
                disposed_fruits.time_since_dispose += get_frame_time();
            }

            for event in self.chart.events[self.event_idx..]
                .iter()
                .filter(|event| self.time >= event.time)
            {
                println!("New Event! {:?}", event.data);
                match &event.data {
                    EventData::Timing { bpm } => self.bpm = *bpm,
                    EventData::Hitsound { kind, volume } => {
                        self.hitsound = kind.clone();
                        self.volume = *volume;
                    }
                }
                self.event_idx += 1;
            }

            let mut defer_delete = Vec::new();

            let audio_dt = self.time - self.prev_time;
            for (idx, &fruit_idx) in self.queued_fruits.iter().enumerate() {
                let fruit = self.chart.fruits[fruit_idx];
                if let Some(result) =
                    self.ruleset
                        .test_hitobject(audio_dt, self.time, fruit, &self.chart)
                {
                    match &result {
                        JudgementResult::Hit((_judgement, details)) => {
                            if !fruit.small {
                                let panning = math::remap(
                                    0.,
                                    self.playfield_width(),
                                    data.panning().0,
                                    data.panning().1,
                                    self.ruleset.position,
                                );

                                let hs_type = match &self.hitsound {
                                    crate::chart::HitSoundKind::Normal => "Normal",
                                    crate::chart::HitSoundKind::Soft => "Soft",
                                    crate::chart::HitSoundKind::Drum => "Drum",
                                    crate::chart::HitSoundKind::Custom(s) => s,
                                };
                                let base_hs_path =
                                    format!("resources/{}/HitSounds/{}", self.chart_name, hs_type);

                                let play_sound = |name: &'static str| {
                                    let data = data.clone();
                                    let base_hs_path = &base_hs_path;
                                    let volume = self.volume;
                                    async move {
                                        let hs_data = data
                                            .audio_cache
                                            .get_sound(
                                                &format!("{}/{}.wav", base_hs_path, name),
                                                data.hitsound_track.id(),
                                            )
                                            .await
                                            .unwrap_or(data.hit_normal.clone());

                                        let mut hitsound =
                                            data.audio.borrow_mut().play(hs_data).unwrap();
                                        hitsound
                                            .set_panning(panning as f64, Tween::default())
                                            .unwrap();
                                        hitsound
                                            .set_volume(volume as f64, Tween::default())
                                            .unwrap();
                                    }
                                };

                                play_sound("Hit").await;
                                if fruit.additions.whistle {
                                    play_sound("Whistle").await;
                                }
                                if fruit.additions.finish {
                                    play_sound("Finish").await;
                                }
                                if fruit.additions.clap {
                                    play_sound("Clap").await;
                                }

                                if fruit.plate_reset {
                                    should_dispose_plate = true;
                                }

                                self.plate.push((details.off, fruit.color));
                            }
                        }
                        JudgementResult::Miss => {
                            if self.recorder.combo >= 8 {
                                data.audio
                                    .borrow_mut()
                                    .play(data.combo_break.clone())
                                    .unwrap();
                            }
                        }
                    }
                    defer_delete.push(idx);
                    self.recorder.register_judgement(result.map_hit(|(j, _)| j));
                }
            }

            let input = if let ReplayType::Playback { input_index, .. } = self.replay_type {
                self.replay
                    .inputs
                    .get(input_index)
                    .copied()
                    .unwrap_or(CatchInput {
                        left: false,
                        right: false,
                        dash: false,
                    })
            } else {
                CatchInput {
                    left: is_key_down(binds.left),
                    right: is_key_down(binds.right),
                    dash: is_key_down(binds.dash),
                }
            };

            self.ruleset.update(
                get_frame_time(),
                input,
                &defer_delete
                    .iter()
                    .map(|&idx| self.chart.fruits[self.queued_fruits[idx]])
                    .collect::<Vec<_>>(),
            );

            match &mut self.replay_type {
                ReplayType::Record => {
                    if self.replay.inputs.len() % 10 == 0 {
                        self.replay.sync_frames.push(ReplaySyncFrame {
                            time: self.time,
                            data: self.ruleset.generate_sync_frame(),
                            input_index: self.replay.inputs.len() as u32,
                        })
                    }
                    self.replay.inputs.push(input);
                }
                ReplayType::Playback {
                    input_index,
                    sync_frame_index,
                } => {
                    *input_index += 1;
                    if let Some(next_sync_frame) = self.replay.sync_frames.get(*sync_frame_index) {
                        if self.time >= next_sync_frame.time {
                            *input_index = next_sync_frame.input_index as usize;
                            self.ruleset.handle_sync_frame(&next_sync_frame.data);
                            *sync_frame_index += 1;
                        }
                    }
                }
            }

            for idx in defer_delete.into_iter().rev() {
                self.queued_fruits.remove(idx);
            }
        }

        if self.plate.len() >= data.max_stack.get() as usize {
            should_dispose_plate = true;
        }

        if should_dispose_plate {
            self.dispose_plate();
        }

        if self.queued_fruits.is_empty() && !self.ended {
            self.fade_out -= get_frame_time();

            if self.fade_out <= 0. {
                let diff_id = data.state().difficulty().id;
                let score = self.recorder.to_score(diff_id);
                if score.passed {
                    data.state_mut().leaderboard.submit_score(&score).await;
                }

                data.send_server(ClientPacket::Submit(score.clone()));

                let map_title = data.state().chart.title.clone();
                let diff_title = data.state().difficulty().name.clone();

                data.broadcast(GameMessage::change_screen(
                    ResultScreen::<CatchRuleset>::new(
                        score,
                        self.replay.clone(),
                        map_title,
                        diff_title,
                    ),
                ));

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
            data.broadcast(GameMessage::change_screen(
                SelectScreen::new(data.clone()).await,
            ));
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
            self.playfield_to_screen_x(0., data.clone()) + 2. / 2.,
            0.,
            self.playfield_to_screen_x(0., data.clone()) + 2. / 2.,
            screen_height(),
            2.,
            RED,
        );

        draw_line(
            self.playfield_to_screen_x(self.playfield_width(), data.clone()) + 2. / 2.,
            0.,
            self.playfield_to_screen_x(self.playfield_width(), data.clone()) + 2. / 2.,
            screen_height(),
            2.,
            RED,
        );

        let fruit_travel_distance =
            self.fruit_y(self.time, 0., 1.) - self.fruit_y(self.prev_time, 0., 1.);
        let catcher_hitbox = Rect::new(
            self.ruleset.position - self.chart.catcher_width / 2.,
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
                fruit.fall_multiplier,
            );

            let mut radius = self.chart.fruit_radius * self.scale(data.clone());
            if fruit.small {
                radius /= 2.0;
            }

            if y + radius <= 0. {
                // queued_fruits are in spawn/hit order currently.
                // I may change it in the future.
                // but for now this exists to improve performance.

                // using `break` breaks SV.
                continue;
            }

            let color = if fruit.hyper.is_some() {
                RED
            } else {
                self.drawable_fruit_color(fruit.color)
            };
            draw_texture_ex(
                data.fruit,
                self.playfield_to_screen_x(fruit.position, data.clone()) - radius,
                y - radius,
                color,
                DrawTextureParams {
                    dest_size: Some(vec2(radius * 2., radius * 2.)),
                    ..Default::default()
                },
            );
            if self.show_debug_hitbox {
                let fruit_hitbox = Rect::new(
                    fruit.position - self.chart.fruit_radius,
                    self.fruit_y(self.time, fruit.time, fruit.fall_multiplier)
                        - fruit_travel_distance / 2.,
                    self.chart.fruit_radius * 2.,
                    fruit_travel_distance,
                );
                let prev_fruit_hitbox = Rect::new(
                    fruit.position - self.chart.fruit_radius,
                    self.fruit_y(self.prev_time, fruit.time, fruit.fall_multiplier)
                        - fruit_travel_distance / 2.,
                    self.chart.fruit_radius * 2.,
                    fruit_travel_distance,
                );

                draw_rectangle(
                    self.playfield_to_screen_x(fruit_hitbox.x, data.clone()),
                    fruit_hitbox.y,
                    fruit_hitbox.w * self.scale(data.clone()),
                    fruit_hitbox.h,
                    BLUE,
                );
                draw_rectangle(
                    self.playfield_to_screen_x(prev_fruit_hitbox.x, data.clone()),
                    prev_fruit_hitbox.y,
                    prev_fruit_hitbox.w * self.scale(data.clone()),
                    prev_fruit_hitbox.h,
                    GREEN,
                );
            }
        }
        if self.show_debug_hitbox {
            draw_rectangle(
                self.playfield_to_screen_x(catcher_hitbox.x, data.clone()),
                catcher_hitbox.y,
                catcher_hitbox.w * self.scale(data.clone()),
                catcher_hitbox.h,
                RED,
            );
        }

        let catcher_position = self.playfield_to_screen_x(self.ruleset.position, data.clone())
            - self.chart.catcher_width * self.scale(data.clone()) / 2.;

        let catcher_sprite_ratio = data.catcher.width() / data.catcher.height();
        let drawable_catcher_width = self.chart.catcher_width * self.scale(data.clone());
        let drawable_catcher_height = drawable_catcher_width / catcher_sprite_ratio;

        let radius = self.chart.fruit_radius * self.scale(data.clone());
        let plate_y = self.catcher_y() - drawable_catcher_height / 2. + radius;
        for (idx, &(off, color)) in self.plate.iter().enumerate() {
            draw_texture_ex(
                data.fruit,
                self.playfield_to_screen_x(self.ruleset.position, data.clone()) - radius
                    + drawable_catcher_width * off / 2.,
                plate_y - (radius * 2.) * 0.1 * idx as f32,
                self.drawable_fruit_color(color),
                DrawTextureParams {
                    dest_size: Some(vec2(radius * 2., radius * 2.)),
                    ..Default::default()
                },
            );
        }

        draw_texture_ex(
            data.catcher,
            catcher_position,
            self.catcher_y(),
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(drawable_catcher_width, drawable_catcher_height)),
                ..Default::default()
            },
        );

        for disposed_fruits in &self.disposed_fruits {
            for fruit in &disposed_fruits.fruits {
                let y = plate_y + fruit.gravity * disposed_fruits.time_since_dispose.powi(2);
                let x_offset = fruit.x_speed * disposed_fruits.time_since_dispose;
                draw_texture_ex(
                    data.fruit,
                    self.playfield_to_screen_x(disposed_fruits.position, data.clone()) - radius
                        + x_offset,
                    y,
                    self.drawable_fruit_color(fruit.color),
                    DrawTextureParams {
                        dest_size: Some(vec2(radius * 2., radius * 2.)),
                        ..Default::default()
                    },
                );
            }
        }

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
