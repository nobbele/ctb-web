use crate::{
    azusa::{ClientPacket, ServerPacket},
    cache::Cache,
    chat::Chat,
    config::KeyBinds,
    leaderboard::Leaderboard,
    log::LogEndpoint,
    promise::PromiseExecutor,
};
use async_trait::async_trait;
use kira::{instance::handle::InstanceHandle, manager::AudioManager, sound::handle::SoundHandle};
use macroquad::prelude::*;
use std::cell::{Cell, Ref, RefCell, RefMut};

use self::game::{GameMessage, SharedGameData};

pub mod game;
pub mod gameplay;
pub mod overlay;
pub mod result;
pub mod select;
pub mod setup;
pub mod visualizer;

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

pub fn get_charts() -> Vec<ChartInfo> {
    vec![
        ChartInfo {
            id: 1,
            title: "Kizuato".to_string(),
            difficulties: vec![
                DifficultyInfo {
                    id: 1,
                    name: "Platter".to_string(),
                },
                DifficultyInfo {
                    id: 2,
                    name: "Ascendance's Rain".to_string(),
                },
            ],
        },
        ChartInfo {
            id: 2,
            title: "Padoru".to_string(),
            difficulties: vec![
                DifficultyInfo {
                    id: 3,
                    name: "Salad".to_string(),
                },
                DifficultyInfo {
                    id: 4,
                    name: "Platter".to_string(),
                },
            ],
        },
        ChartInfo {
            id: 3,
            title: "Troublemaker".to_string(),
            difficulties: vec![
                DifficultyInfo {
                    id: 5,
                    name: "Cup".to_string(),
                },
                DifficultyInfo {
                    id: 6,
                    name: "tocean's Salad".to_string(),
                },
                DifficultyInfo {
                    id: 7,
                    name: "Platter".to_string(),
                },
                DifficultyInfo {
                    id: 8,
                    name: "MBomb's Light Rain".to_string(),
                },
                DifficultyInfo {
                    id: 9,
                    name: "Equim's Rain".to_string(),
                },
                DifficultyInfo {
                    id: 10,
                    name: "Kagari's Himedose".to_string(),
                },
            ],
        },
    ]
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
    pub music: InstanceHandle,
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

    pub general: LogEndpoint,
    pub network: LogEndpoint,
    pub audio_performance: LogEndpoint,

    pub audio_cache: Cache<SoundHandle>,
    pub image_cache: Cache<Texture2D>,

    time: Cell<f32>,
    predicted_time: Cell<f32>,
    background: Cell<Option<Texture2D>>,

    state: RefCell<GameState>,
    promises: RefCell<PromiseExecutor>,
    packet_tx: flume::Sender<ClientPacket>,
    game_tx: flume::Sender<GameMessage>,
}

impl GameData {
    // TODO Improve name.
    pub fn broadcast(&self, msg: GameMessage) {
        self.game_tx.send(msg).unwrap();
    }

    // TODO Improve name.
    pub fn send_server(&self, msg: ClientPacket) {
        self.packet_tx.send(msg).unwrap();
    }

    pub fn time(&self) -> f32 {
        self.time.get()
    }

    pub fn predicted_time(&self) -> f32 {
        self.predicted_time.get()
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
}
