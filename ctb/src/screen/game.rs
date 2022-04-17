use super::{
    overlay::{self, Overlay, OverlayEnum},
    select::SelectScreen,
    setup::SetupScreen,
    visualizer::Visualizer,
    ChartInfo, DifficultyInfo, GameData, GameState, Screen,
};
use crate::{
    azusa::{Azusa, ClientPacket, ServerPacket},
    cache::Cache,
    chat,
    config::{get_value, set_value, KeyBinds},
    leaderboard::Leaderboard,
    log::{LogType, Logger},
    log_to,
    promise::PromiseExecutor,
};
use kira::{
    instance::{
        InstanceLoopStart, InstanceSettings, InstanceState, PauseInstanceSettings,
        ResumeInstanceSettings, StopInstanceSettings,
    },
    manager::{AudioManager, AudioManagerSettings},
    sound::handle::SoundHandle,
};
use macroquad::prelude::*;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

pub enum GameMessage {
    ChangeScreen(Box<dyn Screen>),
    UpdateMusic { handle: SoundHandle, looping: bool },
    PauseMusic,
    ResumeMusic,
    SetMasterVolume(f32),
    SetHitsoundVolume(f32),
}

impl GameMessage {
    pub fn change_screen<S: Screen + 'static>(screen: S) -> Self {
        GameMessage::ChangeScreen(Box::new(screen))
    }

    pub fn update_music(handle: SoundHandle) -> Self {
        GameMessage::UpdateMusic {
            handle,
            looping: false,
        }
    }

    pub fn update_music_looped(handle: SoundHandle) -> Self {
        GameMessage::UpdateMusic {
            handle,
            looping: true,
        }
    }
}

pub type SharedGameData = Rc<GameData>;

pub struct Game {
    pub logger: Logger,
    pub data: SharedGameData,
    screen: Box<dyn Screen>,
    overlay: Option<OverlayEnum>,
    azusa: Option<Azusa>,
    prev_time: f32,
    audio_deltas: ConstGenericRingBuffer<f32, 8>,
    packet_rx: flume::Receiver<ClientPacket>,
    game_rx: flume::Receiver<GameMessage>,
    last_ping: f64,
    sent_ping: bool,
    volume: f32,
}

impl Game {
    pub async fn new() -> Self {
        let mut logger = Logger::new(Duration::from_secs(2));
        let general = logger
            .init_endpoint(LogType::General)
            .path("data/general.log")
            .build();
        let network = logger
            .init_endpoint(LogType::Network)
            //.path("data/network.log")
            .print(true)
            .build();
        let audio_performance = logger
            .init_endpoint(LogType::AudioPerformance)
            //.path("data/audio_performance.log")
            .print(false)
            .build();

        log_to!(general, "Welcome to CTB-Web!");

        let mut audio = AudioManager::new(AudioManagerSettings::default()).unwrap();

        let leaderboard = Leaderboard::new().await;

        let audio_cache = Cache::new("data/cache/audio");
        let image_cache = Cache::new("data/cache/image");

        let mut sound = audio_cache
            .get_sound(&mut audio, "resources/Kizuato/audio.wav")
            .await;

        let instance = sound.play(InstanceSettings::new().volume(0.)).unwrap();

        let first_time = get_value::<bool>("first_time").unwrap_or(true);
        let binds = get_value::<KeyBinds>("binds").unwrap_or(KeyBinds {
            right: KeyCode::D,
            left: KeyCode::A,
            dash: KeyCode::RightShift,
        });
        let token = get_value::<uuid::Uuid>("token");

        let panning = get_value::<(f32, f32)>("panning").unwrap_or((0.25, 0.75));
        let master_volume = get_value("master_volume").unwrap_or(0.25);
        let hitsound_volume = get_value("hitsound_volume").unwrap_or(1.0);

        let (packet_tx, packet_rx) = flume::unbounded();
        let (game_tx, game_rx) = flume::unbounded();

        let combo_break = audio_cache
            .get_sound(&mut audio, "resources/combobreak.wav")
            .await;

        let hit_normal = audio_cache
            .get_sound(&mut audio, "resources/hitnormal.wav")
            .await;

        let data = Rc::new(GameData {
            audio_cache,
            image_cache,
            button: load_texture("resources/button.png").await.unwrap(),
            catcher: load_texture("resources/catcher.png").await.unwrap(),
            fruit: load_texture("resources/fruit.png").await.unwrap(),
            default_background: load_texture("resources/default-bg.png").await.unwrap(),
            combo_break: RefCell::new(combo_break),
            hit_normal: RefCell::new(hit_normal),
            audio: RefCell::new(audio),
            state: RefCell::new(GameState {
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
                chat: chat::Chat::new(),
            }),
            time: Cell::new(0.),
            predicted_time: Cell::new(0.),
            background: Cell::new(None),
            panning: Cell::new(panning),
            master_volume: Cell::new(master_volume),
            hitsound_volume: Cell::new(hitsound_volume),
            locked_input: Cell::new(false),
            promises: RefCell::new(PromiseExecutor::new()),
            packet_tx,
            game_tx,
            general,
            network,
            audio_performance,
        });

        let azusa = if let Some(token) = token {
            Some(Azusa::new(data.clone(), token).await)
        } else {
            set_value("token", "SET TOKEN HERE");
            None
        };

        Game {
            logger,
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
            volume: master_volume,
        }
    }

    pub async fn update(&mut self) {
        self.data.promises().poll();

        let time = self.data.state().music.position() as f32;
        let playing = matches!(
            self.data.state().music.state(),
            InstanceState::Playing | InstanceState::Stopping | InstanceState::Pausing(_)
        );
        self.data.time.set(time);

        let delta = time - self.prev_time;
        self.prev_time = time;

        let avg_delta = self.audio_deltas.iter().sum::<f32>() / self.audio_deltas.len() as f32;
        // If there was no change in time between this and the previous frame, it means the audio took too long to report.
        // But this only makes sense if the music is playing, otherwise it will always have 0 delta.
        if playing {
            if delta == 0. {
                if avg_delta != 0. {
                    let frames_per_audio_frame = avg_delta / get_frame_time();
                    self.data.state_mut().audio_frame_skip = frames_per_audio_frame as u32;
                }

                self.data
                    .predicted_time
                    .set(self.data.predicted_time() + get_frame_time());
            } else {
                let predicted_time = self.data.predicted_time();
                if predicted_time != time {
                    log_to!(
                        self.data.audio_performance,
                        "{} by {:.2}ms (avg: {:.2}) [Skip: {}]",
                        if predicted_time > time {
                            "Overestimated"
                        } else {
                            "Underestimated"
                        },
                        (predicted_time - time) * 1000.,
                        avg_delta * 1000.,
                        self.data.state().audio_frame_skip
                    );
                } else {
                    log_to!(self.data.audio_performance, "Wow! Perfect!");
                }

                self.audio_deltas.push(delta);
                self.data.predicted_time.set(time);
            }
        }

        if let Some(azusa) = &self.azusa {
            if azusa.connected() && is_key_pressed(KeyCode::F9) {
                if let Some(OverlayEnum::Chat(_)) = self.overlay {
                    log_to!(self.data.general, "Closing chat overlay");
                    self.overlay = None;
                } else {
                    log_to!(self.data.general, "Opening chat overlay");
                    self.overlay = Some(OverlayEnum::Chat(overlay::Chat::new()));
                }
            }
        }

        if is_key_pressed(KeyCode::F1) {
            if let Some(OverlayEnum::Settings(_)) = self.overlay {
                log_to!(self.data.general, "Closing settings overlay");
                self.overlay = None;
            } else {
                log_to!(self.data.general, "Opening settings overlay");
                self.overlay = Some(OverlayEnum::Settings(overlay::Settings::new(
                    self.data.clone(),
                )));
            }
        }

        if is_key_pressed(KeyCode::V) {
            self.data
                .broadcast(GameMessage::change_screen(Visualizer::new()));
        }

        if let Some(overlay) = &mut self.overlay {
            overlay.update(self.data.clone()).await;
            self.data.locked_input.set(true);
        }
        self.screen.update(self.data.clone()).await;
        self.data.locked_input.set(false);

        for msg in self.game_rx.drain() {
            match msg {
                GameMessage::ChangeScreen(screen) => self.screen = screen,
                GameMessage::UpdateMusic {
                    mut handle,
                    looping,
                } => {
                    self.data
                        .state_mut()
                        .music
                        .stop(StopInstanceSettings::new())
                        .unwrap();
                    self.data.state_mut().music = handle
                        .play(
                            InstanceSettings::default()
                                .volume(self.volume as f64)
                                .loop_start(if looping {
                                    InstanceLoopStart::Custom(0.0)
                                } else {
                                    InstanceLoopStart::None
                                }),
                        )
                        .unwrap();
                }
                GameMessage::PauseMusic => self
                    .data
                    .state_mut()
                    .music
                    .pause(PauseInstanceSettings::new())
                    .unwrap(),
                GameMessage::ResumeMusic => self
                    .data
                    .state_mut()
                    .music
                    .resume(ResumeInstanceSettings::new())
                    .unwrap(),
                GameMessage::SetMasterVolume(volume) => {
                    self.volume = volume;
                    self.data.master_volume.set(volume);
                    self.data
                        .state_mut()
                        .music
                        .set_volume(volume as f64)
                        .unwrap();
                    set_value("master_volume", volume);
                }
                GameMessage::SetHitsoundVolume(volume) => {
                    self.data.hitsound_volume.set(volume);
                    set_value("hitsound_volume", volume);
                }
            }
        }

        if let Some(azusa) = &mut self.azusa {
            if azusa.connected() {
                for msg in self.packet_rx.drain() {
                    azusa.send(&msg);
                }

                let time_since_ping = get_time() - self.last_ping;
                if time_since_ping > 15.0 && !self.sent_ping {
                    azusa.send(&ClientPacket::Ping);
                    self.sent_ping = true;
                }
                if time_since_ping > 30.0 && azusa.connected() {
                    azusa.set_connected(false);
                }
            }

            for msg in azusa.receive() {
                self.screen.handle_packet(self.data.clone(), &msg);
                match msg {
                    ServerPacket::Connected => {
                        log_to!(self.data.network, "Connected to Azusa!");
                        azusa.set_connected(true);
                    }
                    ServerPacket::Echo(s) => {
                        log_to!(self.data.network, "Azusa says '{}'", s);
                    }
                    ServerPacket::Pong => {
                        self.last_ping = get_time();
                        self.sent_ping = false;
                    }
                    ServerPacket::Chat(packet) => self.data.state_mut().chat.handle_packet(packet),
                    _ => {}
                }
            }
        }

        std::iter::from_fn(get_char_pressed).for_each(drop);

        self.logger.flush();
    }

    pub fn draw(&self) {
        self.screen.draw(self.data.clone());
        if let Some(overlay) = &self.overlay {
            overlay.draw(self.data.clone());
        }
    }
}
