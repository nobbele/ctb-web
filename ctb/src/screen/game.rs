use super::{
    overlay::{chat::ChatOverlay, Overlay},
    select::SelectScreen,
    setup::SetupScreen,
    visualizer::Visualizer,
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

pub enum GameMessage {
    ChangeScreen(Box<dyn Screen>),
}

impl GameMessage {
    pub fn change_screen<S: Screen + 'static>(screen: S) -> Self {
        GameMessage::ChangeScreen(Box::new(screen))
    }
}

pub struct Game {
    pub data: Arc<GameData>,
    screen: Box<dyn Screen>,
    overlay: Option<ChatOverlay>,
    azusa: Azusa,
    prev_time: f32,
    audio_deltas: ConstGenericRingBuffer<f32, 8>,
    packet_rx: flume::Receiver<ClientPacket>,
    game_rx: flume::Receiver<GameMessage>,
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
        let token = get_value::<uuid::Uuid>("token");

        let azusa = Azusa::new().await;
        if let Some(token) = token {
            azusa.send(&ClientPacket::Login(token));
        }

        let (packet_tx, packet_rx) = flume::unbounded();
        let (game_tx, game_rx) = flume::unbounded();

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

                time: 0.,
                predicted_time: 0.,
            }),
            exec,
            packet_tx,
            game_tx,
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
            audio_deltas: ConstGenericRingBuffer::new(),
            packet_rx,
            game_rx,
            last_ping: get_time(),
            sent_ping: false,
        }
    }

    pub async fn update(&mut self) {
        let time = self.data.state.lock().music.position() as f32;
        self.data.state.lock().time = time;

        let delta = time - self.prev_time;
        self.prev_time = time;

        if delta == 0. {
            let avg_delta = self.audio_deltas.iter().sum::<f32>() / self.audio_deltas.len() as f32;
            if avg_delta != 0. {
                let frames_per_audio_frame = avg_delta / get_frame_time();
                self.data.state.lock().audio_frame_skip = frames_per_audio_frame as u32;
            }

            self.data.state.lock().predicted_time += get_frame_time();
        } else {
            /*
            use ringbuffer::{RingBuffer, RingBufferExt};
            info!(
                "Off by an average of {:.1}ms",
                self.prediction_result.iter().sum::<f32>() / self.prediction_result.len() as f32
                    * 1000.
            );*/
            // Print prediction error
            //let audio_frame_skip = data.state.lock().audio_frame_skip;
            /*if audio_frame_skip != 0 {
                let audio_frame_time = get_frame_time() * audio_frame_skip as f32;
                let prediction_off = prediction_delta / audio_frame_time;
                info!(
                    "[{:.2} ({:.2})] Off by {:.2}% ({:.2}ms) (Predicted: {:.2}ms, Actual: {:.2}ms) (frame skip: {})",
                    get_time() * 1000.,
                    get_frame_time() * audio_frame_skip as f32 * 1000.,
                    prediction_off * 100.,
                    prediction_delta * 1000.,
                    self.predicted_time * 1000.,
                    self.time * 1000.,
                    audio_frame_skip
                );
            }
            if prediction_delta < 0. {
                info!(
                    "Overcompensated by {}ms",
                    (-prediction_delta * 1000.).round() as i32
                );
            }*/
            self.audio_deltas.push(delta);
            self.data.state.lock().predicted_time = time;
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

        if is_key_pressed(KeyCode::V) {
            self.data
                .broadcast(GameMessage::change_screen(Visualizer::new()));
        }

        self.screen.update(self.data.clone()).await;
        if let Some(overlay) = &mut self.overlay {
            overlay.update(self.data.clone()).await;
        }

        for msg in self.game_rx.drain() {
            match msg {
                GameMessage::ChangeScreen(s) => self.screen = s,
            }
        }

        for msg in self.packet_rx.drain() {
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
