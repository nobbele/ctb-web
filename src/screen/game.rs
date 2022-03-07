use super::{
    overlay::{chat::ChatOverlay, Overlay},
    select::SelectScreen,
    setup::SetupScreen,
    ChartInfo, DifficultyInfo, GameData, GameState, Screen,
};
use crate::{
    azusa::{Azusa, ClientPacket, ServerPacket},
    cache::Cache,
    chat::Chat,
    config::{get_value, KeyBinds},
    leaderboard::Leaderboard,
    promise::PromiseExecutor,
};
use kira::{
    instance::{InstanceSettings, StopInstanceSettings},
    manager::{AudioManager, AudioManagerSettings},
};
use macroquad::prelude::*;
use parking_lot::Mutex;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};
use std::sync::Arc;

pub struct Game {
    pub data: Arc<GameData>,
    screen: Box<dyn Screen>,
    overlay: Option<ChatOverlay>,
    azusa: Azusa,
    prev_time: f32,
    audio_frame_skip_counter: u32,
    audio_frame_skips: ConstGenericRingBuffer<u32, 4>,
    packet_chan: flume::Receiver<ClientPacket>,
    last_ping: f64,
    sent_ping: bool,
}

impl Game {
    pub async fn new(exec: Mutex<PromiseExecutor>) -> Self {
        let mut audio = AudioManager::new(AudioManagerSettings::default()).unwrap();

        let leaderboard = Leaderboard::new().await;

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

        let azusa = Azusa::new().await;
        azusa.send(&ClientPacket::Login);

        let (tx, rx) = flume::unbounded();

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
                chart: ChartInfo {
                    id: 0,
                    title: "NULL".to_owned(),
                    difficulties: vec![DifficultyInfo {
                        id: 0,
                        name: "NULL".to_owned(),
                    }],
                },
                difficulty_idx: 0,
                leaderboard,
                chat: Chat::new(),
            }),
            exec,
            packet_chan: tx,
        });

        Game {
            screen: if first_time {
                Box::new(SetupScreen::new())
            } else {
                Box::new(SelectScreen::new(data.clone()))
            },
            overlay: None,
            data,
            azusa,
            prev_time: 0.,
            audio_frame_skip_counter: 0,
            audio_frame_skips: ConstGenericRingBuffer::new(),
            packet_chan: rx,
            last_ping: get_time(),
            sent_ping: false,
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

        if is_key_pressed(KeyCode::F9) {
            if self.overlay.is_some() {
                println!("Closing chat overlay");
                self.overlay = None;
            } else {
                println!("Opening chat overlay");
                self.overlay = Some(ChatOverlay::new());
            }
        }

        self.screen.update(self.data.clone()).await;
        if let Some(overlay) = &mut self.overlay {
            overlay.update(self.data.clone()).await;
        }

        if let Some(queued_screen) = self.data.state.lock().queued_screen.take() {
            self.screen = queued_screen;
        }

        for msg in self.packet_chan.drain() {
            self.azusa.send(&msg);
        }

        let time_since_ping = get_time() - self.last_ping;
        if time_since_ping > 15.0 && !self.sent_ping {
            self.azusa.send(&ClientPacket::Ping);
            self.sent_ping = true;
        }
        if time_since_ping > 30.0 && self.azusa.connected() {
            self.azusa.set_connected(false);
        }

        for msg in self.azusa.receive() {
            self.screen.handle_packet(self.data.clone(), &msg);
            match msg {
                ServerPacket::Connected => {
                    info!("Connected to Azusa!");
                    self.azusa.set_connected(true);
                }
                ServerPacket::Echo(s) => {
                    info!("Azusa says '{}'", s);
                }
                ServerPacket::Pong => {
                    self.last_ping = get_time();
                    self.sent_ping = false;
                }
                ServerPacket::Chat(packet) => self.data.state.lock().chat.handle_packet(packet),
                _ => {}
            }
        }
    }

    pub fn draw(&self) {
        self.screen.draw(self.data.clone());
        if let Some(overlay) = &self.overlay {
            overlay.draw(self.data.clone());
        }
    }
}
