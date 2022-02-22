use std::sync::Arc;

use crate::{
    cache::Cache,
    config::{get_value, KeyBinds},
    PromiseExecutor,
};

use self::{select::SelectScreen, setup::SetupScreen};
use async_trait::async_trait;
use kira::{
    instance::{handle::InstanceHandle, InstanceSettings, StopInstanceSettings},
    manager::{AudioManager, AudioManagerSettings},
    sound::handle::SoundHandle,
};
use macroquad::prelude::*;
use parking_lot::Mutex;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};

pub mod gameplay;
pub mod result;
pub mod select;
pub mod setup;

#[async_trait(?Send)]
trait Screen {
    async fn update(&mut self, data: Arc<GameData>);
    fn draw(&self, data: Arc<GameData>);
}

pub struct GameState {
    music: InstanceHandle,
    background: Option<Texture2D>,
    queued_screen: Option<Box<dyn Screen>>,
    audio_frame_skip: u32,
    binds: KeyBinds,
}

pub struct GameData {
    pub audio: Mutex<AudioManager>,
    pub catcher: Texture2D,
    pub fruit: Texture2D,
    pub button: Texture2D,

    pub audio_cache: Cache<SoundHandle>,
    pub image_cache: Cache<Texture2D>,

    pub state: Mutex<GameState>,
    pub exec: Arc<Mutex<PromiseExecutor>>,
}

pub struct Game {
    data: Arc<GameData>,
    screen: Box<dyn Screen>,

    prev_time: f32,
    audio_frame_skip_counter: u32,
    audio_frame_skips: ConstGenericRingBuffer<u32, 4>,
}

impl Game {
    pub async fn new(exec: Arc<Mutex<PromiseExecutor>>) -> Self {
        let mut audio = AudioManager::new(AudioManagerSettings::default()).unwrap();

        let audio_cache = Cache::new("data/cache/audio");
        let image_cache = Cache::new("data/cache/image");

        let mut sound = audio_cache
            .get_sound(&mut audio, "resources/Kizuato/audio.wav")
            .await;

        let mut instance = sound.play(InstanceSettings::default().volume(0.5)).unwrap();
        instance.stop(StopInstanceSettings::new()).unwrap();

        let first_time = get_value::<bool>("first_time").unwrap_or(true);

        let binds = get_value::<KeyBinds>("binds").unwrap_or(KeyBinds {
            right: KeyCode::D,
            left: KeyCode::A,
            dash: KeyCode::RightShift,
        });

        let data = Arc::new(GameData {
            audio_cache,
            image_cache,
            button: load_texture("resources/button.png").await.unwrap(),
            catcher: load_texture("resources/catcher.png").await.unwrap(),
            fruit: load_texture("resources/fruit.png").await.unwrap(),
            audio: Mutex::new(audio),
            state: Mutex::new(GameState {
                background: None,
                music: instance,
                queued_screen: None,
                audio_frame_skip: 0,
                binds,
            }),
            exec,
        });

        Game {
            screen: if first_time {
                Box::new(SetupScreen::new())
            } else {
                Box::new(SelectScreen::new(data.clone()))
            },
            data,

            prev_time: 0.,
            audio_frame_skip_counter: 0,
            audio_frame_skips: ConstGenericRingBuffer::new(),
        }
    }

    pub async fn update(&mut self) {
        let time = self.data.state.lock().music.position() as f32;
        let delta = time - self.prev_time;
        self.prev_time = time;
        if delta == 0. {
            self.audio_frame_skip_counter += 1;
        } else {
            self.audio_frame_skips.push(self.audio_frame_skip_counter);
            self.data.state.lock().audio_frame_skip =
                self.audio_frame_skips.iter().sum::<u32>() / self.audio_frame_skips.len() as u32;
            self.audio_frame_skip_counter = 0;
        }
        self.screen.update(self.data.clone()).await;
        if let Some(queued_screen) = self.data.state.lock().queued_screen.take() {
            self.screen = queued_screen;
        }
    }

    pub fn draw(&self) {
        self.screen.draw(self.data.clone());
    }
}
