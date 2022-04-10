use crate::{
    azusa::{ClientPacket, ServerPacket},
    cache::Cache,
    chat::Chat,
    config::KeyBinds,
    leaderboard::Leaderboard,
    promise::PromiseExecutor,
};
use async_trait::async_trait;
use kira::{instance::handle::InstanceHandle, manager::AudioManager, sound::handle::SoundHandle};
use macroquad::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;

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
    async fn update(&mut self, data: Arc<GameData>);
    fn draw(&self, data: Arc<GameData>);
    fn handle_packet(&mut self, data: Arc<GameData>, packet: &ServerPacket) {
        drop((data, packet));
    }
}

pub struct GameState {
    pub chart: ChartInfo,
    pub difficulty_idx: usize,
    pub music: InstanceHandle,
    pub background: Option<Texture2D>,
    pub queued_screen: Option<Box<dyn Screen>>,
    pub audio_frame_skip: u32,
    pub binds: KeyBinds,

    pub time: f32,
    pub predicted_time: f32,

    pub leaderboard: Leaderboard,
    pub chat: Chat,
}

pub struct GameData {
    pub audio: Mutex<AudioManager>,
    pub catcher: Texture2D,
    pub fruit: Texture2D,
    pub button: Texture2D,

    pub audio_cache: Cache<SoundHandle>,
    pub image_cache: Cache<Texture2D>,

    pub state: Mutex<GameState>,
    pub exec: Mutex<PromiseExecutor>,
    pub packet_chan: flume::Sender<ClientPacket>,
}
