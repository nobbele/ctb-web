#![feature(once_cell)]
#![allow(clippy::eq_op)]
use std::{
    io::Cursor,
    ops::Add,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use kira::{
    instance::{
        handle::InstanceHandle, InstanceSettings, PauseInstanceSettings, ResumeInstanceSettings,
        StopInstanceSettings,
    },
    manager::{AudioManager, AudioManagerSettings},
    sound::{Sound, SoundSettings},
};
use macroquad::prelude::*;
use num_format::{Locale, ToFormattedString};

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

#[derive(Debug, Copy, Clone)]
struct Fruit {
    position: f32,
    time: f32,
    hyper: Option<f32>,
}

impl Fruit {
    pub fn from_hitobject(hitobject: &osu_types::HitObject) -> Self {
        Fruit {
            position: hitobject.position.0 as f32,
            time: hitobject.time as f32 / 1000.,
            hyper: None,
        }
    }
}

struct Chart {
    fruits: Vec<Fruit>,
    fall_time: f32,
    fruit_radius: f32,
    catcher_width: f32,
}

impl Chart {
    pub fn from_beatmap(beatmap: &osu_parser::Beatmap) -> Self {
        let mut fruits = Vec::with_capacity(beatmap.hit_objects.len());
        for (idx, hitobject) in beatmap.hit_objects.iter().enumerate() {
            let mut fruit = Fruit::from_hitobject(hitobject);

            // If you can't get to the center of the next fruit in time, we need to give the player some extra speed.
            // TODO use same implementation as osu!catch.
            if let Some(next_hitobject) = beatmap.hit_objects.get(idx + 1) {
                let next_fruit = Fruit::from_hitobject(next_hitobject);
                let dist = next_fruit.position - fruit.position;
                let time = next_fruit.time - fruit.time;
                let required_time = dist / catcher_speed(true, 1.);
                if required_time > time {
                    fruit.hyper = Some(required_time / time);
                };
            }

            fruits.push(fruit);
        }

        Chart {
            fruits,
            fall_time: osu_utils::ar_to_ms(beatmap.info.difficulty.ar) / 1000.,
            fruit_radius: osu_utils::cs_to_px(beatmap.info.difficulty.cs),
            catcher_width: {
                let scale = 1. - 0.7 * (beatmap.info.difficulty.cs - 5.) / 5.;
                106.75 * scale * 0.8
            },
        }
    }
}

struct ScoreRecorder {
    combo: u32,
    top_combo: u32,
    max_combo: u32,

    hit_count: u32,
    miss_count: u32,

    // This needs to be tracked separately cause of floating point imprecision.
    internal_score: f32,
    // Max = 1,000,000
    score: u32,
    accuracy: f32,
}

impl ScoreRecorder {
    pub fn new(max_combo: u32) -> Self {
        ScoreRecorder {
            combo: 0,
            top_combo: 0,
            max_combo,
            hit_count: 0,
            miss_count: 0,
            internal_score: 0.,
            score: 0,
            accuracy: 1.0,
        }
    }

    pub fn register_judgement(&mut self, hit: bool) {
        if hit {
            self.combo += 1;
            self.top_combo = self.top_combo.max(self.combo);

            self.internal_score += self.combo as f32 / self.max_combo as f32;
            self.score = (self.internal_score * 1_000_000. * 2. / (self.max_combo as f32 + 1.))
                .round() as u32;
            self.hit_count += 1;
        } else {
            self.combo = 0;
            self.miss_count += 1;
        }

        self.accuracy = self.hit_count as f32 / (self.hit_count + self.miss_count) as f32;
    }
}

#[test]
fn test_score_recorder_limits() {
    for max_combo in (1..256).step_by(13) {
        dbg!(max_combo);
        let mut recorder = ScoreRecorder::new(max_combo);
        for _ in 0..max_combo {
            recorder.register_judgement(true);
        }
        assert_eq!(recorder.score, 1_000_000);
    }
}

struct Gameplay {
    background: Texture2D,

    recorder: ScoreRecorder,

    time: f32,
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

#[async_trait(?Send)]
trait Screen {
    async fn update(&mut self, data: Arc<GameData>);
    fn draw(&self, data: Arc<GameData>);
}

impl Gameplay {
    pub async fn new(data: Arc<GameData>, map: &str, diff: &str) -> Self {
        let beatmap_data = load_file(&format!("resources/{}/{}.osu", map, diff))
            .await
            .unwrap();
        let beatmap_content = std::str::from_utf8(&beatmap_data).unwrap();
        let beatmap =
            osu_parser::load_content(beatmap_content, osu_parser::BeatmapParseOptions::default())
                .unwrap();
        let chart = Chart::from_beatmap(&beatmap);

        let background = load_texture(&format!("resources/{}/bg.png", map))
            .await
            .unwrap();

        let song = load_file(&format!("resources/{}/audio.wav", map))
            .await
            .unwrap();
        let sound_data =
            Sound::from_wav_reader(Cursor::new(song), SoundSettings::default()).unwrap();
        let mut sound = data.audio.lock().unwrap().add_sound(sound_data).unwrap();
        data.music
            .lock()
            .unwrap()
            .stop(StopInstanceSettings::new())
            .unwrap();
        *data.music.lock().unwrap() = sound.play(InstanceSettings::default().volume(0.5)).unwrap();
        data.music
            .lock()
            .unwrap()
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
            background,
            time: -time_countdown,
            prev_time: -time_countdown,
            recorder: ScoreRecorder::new(chart.fruits.len() as u32),
            position: 256.,
            hyper_multiplier: 1.,
            deref_delete: Vec::new(),
            queued_fruits: (0..chart.fruits.len()).collect(),
            chart,
            show_debug_hitbox: cfg!(debug_assertions),
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

    pub fn catcher_speed(&self) -> f32 {
        catcher_speed(is_key_down(KeyCode::RightShift), self.hyper_multiplier)
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

    pub fn movement_direction(&self) -> i32 {
        is_key_down(KeyCode::D) as i32 - is_key_down(KeyCode::A) as i32
    }
}

#[async_trait(?Send)]
impl Screen for Gameplay {
    async fn update(&mut self, data: Arc<GameData>) {
        let catcher_y = self.catcher_y();

        if !self.started {
            self.prev_time = self.time;
            self.time = -self.time_countdown;
            if self.time_countdown > 0. {
                self.time_countdown -= get_frame_time();
            } else {
                data.music
                    .lock()
                    .unwrap()
                    .resume(ResumeInstanceSettings::new())
                    .unwrap();
                self.started = true;
            }
        } else {
            self.prev_time = self.time;
            self.time = data.music.lock().unwrap().position() as f32;
        }

        self.position = self
            .position
            .add(self.movement_direction() as f32 * self.catcher_speed() * get_frame_time() /*FIXED_DELTA*/)
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
                println!("Miss!");
            }
            if hit || miss {
                self.hyper_multiplier = fruit.hyper.unwrap_or(1.);
            }
        }

        for idx in self.deref_delete.drain(..).rev() {
            self.queued_fruits.remove(idx);
        }

        if is_key_pressed(KeyCode::O) {
            self.show_debug_hitbox = !self.show_debug_hitbox;
        }
        if is_key_pressed(KeyCode::Escape) {
            data.music
                .lock()
                .unwrap()
                .stop(StopInstanceSettings::new())
                .unwrap();
            *data.queued_screen.lock().unwrap() = Some(Box::new(MainMenu::new(data.clone()).await));
        }
    }

    fn draw(&self, data: Arc<GameData>) {
        draw_texture_ex(
            self.background,
            0.,
            0.,
            Color::new(1., 1., 1., 0.2),
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
            draw_texture_ex(
                data.fruit,
                self.playfield_to_screen_x(fruit.position) - self.chart.fruit_radius * self.scale(),
                self.fruit_y(self.time, fruit.time) - self.chart.fruit_radius * self.scale(),
                if fruit.hyper.is_some() { RED } else { WHITE },
                DrawTextureParams {
                    dest_size: Some(vec2(
                        self.chart.fruit_radius * 2. * self.scale(),
                        self.chart.fruit_radius * 2. * self.scale(),
                    )),
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
    }
}

struct Message {
    sender: String,
    data: MessageData,
}

enum MessageData {
    MenuButton(MenuButtonMessage),
    MenuButtonList(MenuButtonListMessage),
}

enum MenuButtonMessage {
    Selected,
    Unselected,
    Hovered,
    Unhovered,
}

trait UiElement {
    fn draw(&self, data: Arc<GameData>);
    fn update(&self, data: Arc<GameData>);
    fn handle_message(&mut self, _message: &Message) {}
}

struct MenuButton {
    id: String,
    title: String,
    rect: Rect,
    tx: flume::Sender<Message>,
    hovered: bool,
    selected: bool,
}
const SELECTED_COLOR: Color = Color::new(1.0, 1.0, 1.0, 1.0);
const HOVERED_COLOR: Color = Color::new(0.5, 0.5, 0.8, 1.0);
const IDLE_COLOR: Color = Color::new(0.5, 0.5, 0.5, 1.0);

impl MenuButton {
    pub fn new(id: String, title: String, rect: Rect, tx: flume::Sender<Message>) -> Self {
        MenuButton {
            id,
            title,
            rect,
            tx,
            hovered: false,
            selected: false,
        }
    }
}

impl UiElement for MenuButton {
    fn draw(&self, data: Arc<GameData>) {
        draw_texture_ex(
            data.button,
            self.rect.x,
            self.rect.y,
            if self.selected {
                SELECTED_COLOR
            } else if self.hovered {
                HOVERED_COLOR
            } else {
                IDLE_COLOR
            },
            DrawTextureParams {
                dest_size: Some(vec2(self.rect.w, self.rect.h)),
                ..Default::default()
            },
        );
        let title_length = measure_text(&self.title, None, 36, 1.);
        draw_text(
            &self.title,
            self.rect.x + self.rect.w / 2. - title_length.width / 2.,
            self.rect.y + self.rect.h / 2. - title_length.height / 2.,
            36.,
            WHITE,
        )
    }

    fn update(&self, _data: Arc<GameData>) {
        if self.rect.contains(mouse_position().into()) {
            if !self.hovered {
                self.tx
                    .send(Message {
                        sender: self.id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Hovered),
                    })
                    .unwrap();
            }
            if is_mouse_button_pressed(MouseButton::Left) {
                self.tx
                    .send(Message {
                        sender: self.id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Selected),
                    })
                    .unwrap();
            }
        } else {
            if self.hovered {
                self.tx
                    .send(Message {
                        sender: self.id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Unhovered),
                    })
                    .unwrap();
            }
        }
    }

    fn handle_message(&mut self, message: &Message) {
        if message.sender == self.id {
            if let MessageData::MenuButton(MenuButtonMessage::Hovered) = message.data {
                self.hovered = true;
            } else if let MessageData::MenuButton(MenuButtonMessage::Unhovered) = message.data {
                self.hovered = false;
            } else if let MessageData::MenuButton(MenuButtonMessage::Selected) = message.data {
                self.selected = true;
            } else if let MessageData::MenuButton(MenuButtonMessage::Unselected) = message.data {
                self.selected = false;
            }
        }
    }
}

enum MenuButtonListMessage {
    Click(usize),
    Selected(usize),
}

struct MenuButtonList {
    id: String,
    buttons: Vec<MenuButton>,
    selected: usize,
    tx: flume::Sender<Message>,
}

impl MenuButtonList {
    pub fn new(
        id: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        titles: &[String],
        tx: flume::Sender<Message>,
    ) -> Self {
        let button_height = h / titles.len() as f32;
        MenuButtonList {
            id: id.clone(),
            buttons: titles
                .iter()
                .enumerate()
                .map(|(idx, title)| {
                    MenuButton::new(
                        format!("{}-{}", &id, idx),
                        title.clone(),
                        Rect::new(x, y + (button_height + 5.) * idx as f32, w, button_height),
                        tx.clone(),
                    )
                })
                .collect(),
            selected: 0,
            tx,
        }
    }
}

impl UiElement for MenuButtonList {
    fn draw(&self, data: Arc<GameData>) {
        for button in &self.buttons {
            button.draw(data.clone());
        }
    }

    fn update(&self, data: Arc<GameData>) {
        for button in &self.buttons {
            button.update(data.clone());
        }

        if is_key_pressed(KeyCode::Down) {
            self.tx
                .send(Message {
                    sender: self.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                        (self.selected + 1) % self.buttons.len(),
                    )),
                })
                .unwrap();
        } else if is_key_pressed(KeyCode::Up) {
            self.tx
                .send(Message {
                    sender: self.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                        (self.selected + self.buttons.len() - 1) % self.buttons.len(),
                    )),
                })
                .unwrap();
        }
    }

    fn handle_message(&mut self, message: &Message) {
        if message.sender == self.id {
            if let MessageData::MenuButtonList(MenuButtonListMessage::Click(idx)) = message.data {
                self.tx
                    .send(Message {
                        sender: self.buttons[self.selected].id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Unselected),
                    })
                    .unwrap();
                self.tx
                    .send(Message {
                        sender: self.buttons[idx].id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Selected),
                    })
                    .unwrap();
            }
        }
        if message.sender.starts_with(&self.id) {
            if let MessageData::MenuButton(MenuButtonMessage::Selected) = message.data {
                for (idx, button) in self.buttons.iter().enumerate() {
                    if button.id == message.sender {
                        self.selected = idx;
                        self.tx
                            .send(Message {
                                sender: self.id.clone(),
                                data: MessageData::MenuButtonList(MenuButtonListMessage::Selected(
                                    idx,
                                )),
                            })
                            .unwrap();
                    } else {
                        button
                            .tx
                            .send(Message {
                                sender: button.id.clone(),
                                data: MessageData::MenuButton(MenuButtonMessage::Unselected),
                            })
                            .unwrap();
                    }
                }
            }
            for button in &mut self.buttons {
                button.handle_message(message);
            }
        }
    }
}

struct MapListing {
    title: String,
    difficulties: Vec<String>,
}

struct MainMenu {
    maps: Vec<MapListing>,
    prev_selected_map: usize,
    selected_map: usize,
    selected_difficulty: usize,

    rx: flume::Receiver<Message>,
    tx: flume::Sender<Message>,
    map_list: MenuButtonList,
    difficulty_list: Option<MenuButtonList>,

    start: MenuButton,
    loading: bool,
}

impl MainMenu {
    pub async fn new(_data: Arc<GameData>) -> Self {
        let (tx, rx) = flume::unbounded();
        let maps = vec![
            MapListing {
                title: "Kizuato".to_string(),
                difficulties: vec!["Platter".to_string(), "Ascendance's Rain".to_string()],
            },
            MapListing {
                title: "Padoru".to_string(),
                difficulties: vec!["Salad".to_string(), "Platter".to_string()],
            },
        ];
        let map_list = MenuButtonList::new(
            "button_list".to_string(),
            0.,
            0.,
            400.,
            100. * maps.len() as f32,
            maps.iter()
                .map(|map| map.title.clone())
                .collect::<Vec<_>>()
                .as_slice(),
            tx.clone(),
        );
        tx.send(Message {
            sender: map_list.id.clone(),
            data: MessageData::MenuButtonList(MenuButtonListMessage::Click(0)),
        })
        .unwrap();
        MainMenu {
            prev_selected_map: usize::MAX,
            selected_map: usize::MAX,
            selected_difficulty: 0,

            maps,
            rx,
            tx: tx.clone(),
            map_list,
            difficulty_list: None,
            start: MenuButton::new(
                "start".to_string(),
                "Start".to_string(),
                Rect::new(
                    screen_width() / 2. - 400. / 2.,
                    screen_height() - 100.,
                    400.,
                    100.,
                ),
                tx,
            ),
            loading: false,
        }
    }
}

#[async_trait(?Send)]
impl Screen for MainMenu {
    async fn update(&mut self, data: Arc<GameData>) {
        if self.selected_map != self.prev_selected_map {
            let song = load_file(&format!(
                "resources/{}/audio.wav",
                self.maps[self.selected_map].title
            ))
            .await
            .unwrap();
            let sound_data =
                Sound::from_wav_reader(Cursor::new(song), SoundSettings::default()).unwrap();
            let mut sound = data.audio.lock().unwrap().add_sound(sound_data).unwrap();
            data.music
                .lock()
                .unwrap()
                .stop(StopInstanceSettings::new())
                .unwrap();
            *data.music.lock().unwrap() =
                sound.play(InstanceSettings::default().volume(0.5)).unwrap();

            let difficulty_list = MenuButtonList::new(
                "difficulty_list".to_string(),
                500.,
                0.,
                400.,
                100. * self.maps[self.selected_map].difficulties.len() as f32,
                self.maps[self.selected_map]
                    .difficulties
                    .iter()
                    .map(|diff| diff.clone())
                    .collect::<Vec<_>>()
                    .as_slice(),
                self.tx.clone(),
            );
            self.tx
                .send(Message {
                    sender: difficulty_list.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::Click(0)),
                })
                .unwrap();
            self.difficulty_list = Some(difficulty_list);

            self.prev_selected_map = self.selected_map;

            self.loading = false;
        }
        for message in self.rx.drain() {
            self.map_list.handle_message(&message);
            if let Some(ref mut difficulty_list) = self.difficulty_list {
                difficulty_list.handle_message(&message);
            }
            self.start.handle_message(&message);
            if message.sender == self.map_list.id {
                if let MessageData::MenuButtonList(MenuButtonListMessage::Selected(idx)) =
                    message.data
                {
                    self.selected_map = idx;
                    self.loading = true;
                }
            }
            if let Some(ref mut difficulty_list) = self.difficulty_list {
                if message.sender == difficulty_list.id {
                    if let MessageData::MenuButtonList(MenuButtonListMessage::Selected(idx)) =
                        message.data
                    {
                        self.selected_difficulty = idx;
                    }
                }
            }
            if message.sender == self.start.id {
                if let MessageData::MenuButton(MenuButtonMessage::Selected) = message.data {
                    let map = &self.maps[self.selected_map];
                    *data.queued_screen.lock().unwrap() = Some(Box::new(
                        Gameplay::new(
                            data.clone(),
                            &map.title,
                            &map.difficulties[self.selected_difficulty],
                        )
                        .await,
                    ));
                }
            }
        }
        self.map_list.update(data.clone());
        if let Some(difficulty_list) = &self.difficulty_list {
            difficulty_list.update(data.clone());
        }
        self.start.update(data.clone());
    }

    fn draw(&self, data: Arc<GameData>) {
        self.map_list.draw(data.clone());
        if let Some(ref list) = self.difficulty_list {
            list.draw(data.clone());
        }
        self.start.draw(data.clone());

        if self.loading {
            let loading_dim = measure_text("Loading...", None, 36, 1.);
            draw_text(
                "Loading...",
                screen_width() / 2. - loading_dim.width / 2.,
                screen_height() / 2. - loading_dim.height / 2.,
                36.,
                WHITE,
            );
        }
    }
}

struct GameData {
    #[allow(dead_code)]
    audio: Mutex<AudioManager>,
    catcher: Texture2D,
    fruit: Texture2D,
    button: Texture2D,

    music: Mutex<InstanceHandle>,
    queued_screen: Mutex<Option<Box<dyn Screen>>>,
}

struct Game {
    data: Arc<GameData>,
    screen: Box<dyn Screen>,
}

impl Game {
    pub async fn new() -> Self {
        let mut audio = AudioManager::new(AudioManagerSettings::default()).unwrap();

        let song = load_file("resources/Kizuato/audio.wav").await.unwrap();
        let sound_data =
            Sound::from_wav_reader(Cursor::new(song), SoundSettings::default()).unwrap();
        let mut sound = audio.add_sound(sound_data).unwrap();
        let mut instance = sound.play(InstanceSettings::default().volume(0.5)).unwrap();
        instance.stop(StopInstanceSettings::new()).unwrap();

        let data = Arc::new(GameData {
            button: load_texture("resources/button.png").await.unwrap(),
            catcher: load_texture("resources/catcher.png").await.unwrap(),
            fruit: load_texture("resources/fruit.png").await.unwrap(),
            audio: Mutex::new(audio),
            music: Mutex::new(instance),
            queued_screen: Mutex::new(None),
        });

        Game {
            screen: Box::new(MainMenu::new(data.clone()).await),
            data,
        }
    }

    pub async fn update(&mut self) {
        self.screen.update(self.data.clone()).await;
        if let Some(queued_screen) = self.data.queued_screen.lock().unwrap().take() {
            self.screen = queued_screen;
        }
    }

    pub fn draw(&self) {
        self.screen.draw(self.data.clone());
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new().await;
    loop {
        game.update().await;

        clear_background(BLACK);
        game.draw();
        next_frame().await
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "CTB Web".to_owned(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}
