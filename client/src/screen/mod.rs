use self::{
    game::{GameMessage, SharedGameData},
    gameplay::Mod,
};
use crate::{
    azusa::{ClientPacket, ServerPacket},
    cache::Cache,
    chat::Chat,
    config::{self, KeyBinds},
    leaderboard::Leaderboard,
    promise::PromiseExecutor,
};
use async_trait::async_trait;
use gluesql::{
    prelude::{Glue, Payload},
    sled_storage::SledStorage,
};
use kira::{
    manager::AudioManager,
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
    track::TrackHandle,
};
use macroquad::prelude::*;
use std::cell::{Cell, Ref, RefCell, RefMut};

pub mod game;
pub mod gameplay;
pub mod overlay;
pub mod result;
pub mod select;
pub mod setup;
pub mod visualizer;

// TODO move game.rs and GameState and GameData into a game module.

#[derive(Debug, Clone)]
pub struct DifficultyInfo {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ChartInfo {
    pub id: u32,
    pub title: String,
    pub difficulties: Vec<DifficultyInfo>,
}

pub fn get_charts(data: SharedGameData) -> Vec<ChartInfo> {
    let mut cache_db = data.chart_db.borrow_mut();
    match cache_db.execute("SELECT id, title FROM charts").unwrap() {
        Payload::Select { labels: _, rows } => rows
            .into_iter()
            .map(|columns| {
                let id: i64 = columns[0].clone().try_into().unwrap();
                let title = columns[1].clone().try_into().unwrap();

                let difficulties = match cache_db
                    .execute(format!(
                        "SELECT id, title FROM difficulties WHERE chart_id = {}",
                        id
                    ))
                    .unwrap()
                {
                    Payload::Select { labels: _, rows } => rows
                        .into_iter()
                        .map(|columns| {
                            let id: i64 = columns[0].clone().try_into().unwrap();
                            let name = columns[1].clone().try_into().unwrap();
                            DifficultyInfo { id: id as _, name }
                        })
                        .collect(),
                    _ => unreachable!(),
                };

                ChartInfo {
                    id: id as _,
                    title,
                    difficulties,
                }
            })
            .collect(),
        _ => unreachable!(),
    }
}

#[async_trait(?Send)]
pub trait Screen {
    async fn update(&mut self, data: SharedGameData);
    fn draw(&self, data: SharedGameData);
    fn handle_packet(&mut self, data: SharedGameData, packet: &ServerPacket) {
        drop((data, packet));
    }
}

pub struct GameState {
    pub chart: ChartInfo,
    pub difficulty_idx: usize,
    pub music: StaticSoundHandle,
    pub audio_frame_skip: u32,
    pub binds: KeyBinds,

    pub leaderboard: Leaderboard,
    pub chat: Chat,
}

impl GameState {
    pub fn difficulty(&self) -> &DifficultyInfo {
        &self.chart.difficulties[self.difficulty_idx]
    }
}

pub struct GameData {
    pub audio: RefCell<AudioManager>,
    pub catcher: Texture2D,
    pub fruit: Texture2D,
    pub button: Texture2D,
    pub default_background: Texture2D,
    pub combo_break: StaticSoundData,
    pub hit_normal: StaticSoundData,

    // Needs to be kept alive.
    #[allow(dead_code)]
    pub hitsound_track: TrackHandle,
    #[allow(dead_code)]
    pub main_track: TrackHandle,

    main_volume: Cell<f32>,
    // Only used to read in settings, otherwise read-only.
    hitsound_volume: Cell<f32>,
    panning: Cell<(f32, f32)>,
    offset: Cell<f32>,

    pub audio_cache: Cache<StaticSoundData>,
    pub image_cache: Cache<Texture2D>,

    time: Cell<f32>,
    predicted_time: Cell<f32>,
    background: Cell<Option<Texture2D>>,

    locked_input: Cell<bool>,

    /// Playfield size as a percent of the screen width \[0; 1\].
    playfield_size: Cell<f32>,
    max_stack: Cell<u32>,

    state: RefCell<GameState>,
    promises: RefCell<PromiseExecutor>,
    packet_tx: flume::Sender<ClientPacket>,
    game_tx: flume::Sender<GameMessage>,

    mods: RefCell<Vec<Mod>>,
    rate: Cell<f32>,

    pub chart_db: RefCell<Glue<gluesql::sled_storage::sled::IVec, SledStorage>>,
}

impl GameData {
    /// Send a message to the game manager.
    // TODO Improve name.
    pub fn broadcast(&self, msg: GameMessage) {
        self.game_tx.send(msg).unwrap();
    }

    /// Send a packet to Azusa.
    // TODO Improve name.
    pub fn send_server(&self, msg: ClientPacket) {
        self.packet_tx.send(msg).unwrap();
    }

    /// Includes offset.
    pub fn time_with_offset(&self) -> f32 {
        self.time.get() + self.offset.get()
    }

    /// Includes offset.
    pub fn predicted_time_with_offset(&self) -> f32 {
        self.predicted_time.get() + self.offset.get()
    }

    pub fn main_volume(&self) -> f32 {
        self.main_volume.get()
    }

    pub fn panning(&self) -> (f32, f32) {
        self.panning.get()
    }

    pub fn set_panning(&self, left: f32, right: f32) {
        assert!(left <= right);
        assert!(left >= 0.0);
        assert!(right <= 1.0);
        self.panning.set((left, right));
        config::set_value("panning", (left, right));
    }

    pub fn promises(&self) -> RefMut<'_, PromiseExecutor> {
        self.promises.borrow_mut()
    }

    pub fn state(&self) -> Ref<'_, GameState> {
        self.state.borrow()
    }

    pub fn state_mut(&self) -> RefMut<'_, GameState> {
        self.state.borrow_mut()
    }

    pub fn background(&self) -> Texture2D {
        self.background.get().unwrap_or(self.default_background)
    }

    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        !self.locked_input.get() && is_key_pressed(key)
    }
}
