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
    promise::{Promise, PromiseExecutor},
    LogType,
};
use aether::log;
use gluesql::{prelude::Glue, sled_storage::SledStorage};
use kira::{
    manager::{AudioManager, AudioManagerSettings},
    sound::static_sound::{PlaybackState, StaticSoundData},
    track::{
        effect::volume_control::{VolumeControlBuilder, VolumeControlHandle},
        TrackBuilder, TrackRoutes,
    },
    tween::Tween,
    PlaybackRate,
};
use macroquad::prelude::*;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
};

pub enum GameMessage {
    ChangeScreen(Box<dyn Screen>),
    LoadScreen(Pin<Box<dyn Future<Output = Box<dyn Screen + Send>>>>),
    UpdateMusic {
        handle: StaticSoundData,
        looping: bool,
    },
    SetMusicRate(f32),
    PauseMusic,
    ResumeMusic,
    SetMainVolume(f32),
    SetHitsoundVolume(f32),
    SetOffset(f32),
    Login {
        username: String,
        password: String,
    },
}

impl GameMessage {
    pub fn change_screen<S: Screen + 'static>(screen: S) -> Self {
        GameMessage::ChangeScreen(Box::new(screen))
    }

    pub fn load_screen<S, F>(screen_fut: F) -> Self
    where
        S: Screen + Send + 'static,
        F: Future<Output = S> + 'static,
    {
        GameMessage::LoadScreen(Box::pin(async {
            let screen = screen_fut.await;
            Box::new(screen) as _
        }))
    }

    pub fn update_music(handle: StaticSoundData) -> Self {
        GameMessage::UpdateMusic {
            handle,
            looping: false,
        }
    }

    pub fn update_music_looped(handle: StaticSoundData) -> Self {
        GameMessage::UpdateMusic {
            handle,
            looping: true,
        }
    }
}

pub type SharedGameData = Rc<GameData>;

pub struct Game {
    _keep: aether::KeepAlive,
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

    hitsound_volume_handle: VolumeControlHandle,
    main_volume_handle: VolumeControlHandle,

    login_request: Option<quad_net::http_request::Request>,
    screen_loading_promise: Option<Promise<()>>,
}

impl Game {
    pub async fn new() -> Self {
        let _keep = if cfg!(target_arch = "wasm32") {
            aether::init()
                .setup(LogType::General, |ep| ep)
                .setup(LogType::Network, |ep| ep)
                .setup(LogType::AudioPerformance, |ep| ep.silent())
                .build()
        } else {
            aether::init()
                .base_path("data/logs")
                .setup(LogType::General, |ep| ep.path("general.log"))
                .setup(LogType::Network, |ep| ep)
                .setup(LogType::AudioPerformance, |ep| {
                    ep.path("audio.log").silent()
                })
                .build()
        };

        aether::log!(LogType::General, "Welcome to CTB-Web!");

        let mut audio = AudioManager::new(AudioManagerSettings::default()).unwrap();

        let leaderboard = Leaderboard::new().await;

        let audio_cache = Cache::new("data/cache/audio");
        let image_cache = Cache::new("data/cache/image");

        let first_time = get_value::<bool>("first_time").unwrap_or(true);
        let binds = get_value::<KeyBinds>("binds").unwrap_or(KeyBinds {
            right: KeyCode::D,
            left: KeyCode::A,
            dash: KeyCode::RightShift,
        });
        let token = get_value::<uuid::Uuid>("token");

        let panning = get_value::<(f32, f32)>("panning").unwrap_or((0.25, 0.75));
        let main_volume = get_value("main_volume").unwrap_or(0.25);
        let hitsound_volume = get_value("hitsound_volume").unwrap_or(1.0);
        let max_stack = get_value("max_stack").unwrap_or(16);
        let playfield_size = get_value("playfield_size").unwrap_or(2. / 3.);

        // Linux usually needs a +30ms offset for compatibility with windows. (I think..)
        let offset = get_value("offset").unwrap_or(if cfg!(unix) { 0.03 } else { 0.0 });

        let (packet_tx, packet_rx) = flume::unbounded();
        let (game_tx, game_rx) = flume::unbounded();

        let (main_track, main_volume_handle) = {
            let mut builder = TrackBuilder::new();
            let volume = builder.add_effect(VolumeControlBuilder::new(main_volume as f64));
            let track = audio.add_sub_track(builder).unwrap();
            (track, volume)
        };

        let (hitsound_track, hitsound_volume_handle) = {
            let mut builder = TrackBuilder::new().routes(TrackRoutes::parent(main_track.id()));
            let volume = builder.add_effect(VolumeControlBuilder::new(hitsound_volume as f64));
            let track = audio.add_sub_track(builder).unwrap();
            (track, volume)
        };

        let files = load_file("resources/Kizuato/files.json").await.unwrap();
        let files: Vec<String> = serde_json::from_slice(&files).unwrap();
        files
            .into_iter()
            .for_each(|path| audio_cache.whitelist(format!("resources/Kizuato/{}", path)));

        let sound = audio_cache
            .get_sound("resources/Kizuato/audio.wav", main_track.id())
            .await
            .unwrap();

        let mut instance = audio.play(sound).unwrap();
        instance.set_volume(0., Tween::default()).unwrap();

        let combo_break = audio_cache
            .get_sound_bypass("resources/combobreak.wav", hitsound_track.id())
            .await
            .unwrap();
        let hit_normal = audio_cache
            .get_sound_bypass("resources/hitnormal.wav", hitsound_track.id())
            .await
            .unwrap();

        let fresh_cache = !std::path::PathBuf::from("data/.chart").exists();
        let mut chart_db = Glue::new(SledStorage::new("data/.chart").unwrap());
        if fresh_cache {
            for stmt in include_str!("../queries/initialize_chart.sql")
                .split(';')
                .filter(|s| !s.is_empty())
            {
                chart_db.execute_async(stmt).await.unwrap();
            }
        }

        let data = Rc::new(GameData {
            audio_cache,
            image_cache,
            button: load_texture("resources/button.png").await.unwrap(),
            catcher: load_texture("resources/catcher.png").await.unwrap(),
            fruit: load_texture("resources/fruit.png").await.unwrap(),
            default_background: load_texture("resources/default-bg.png").await.unwrap(),
            combo_break,
            hit_normal,
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
            main_volume: Cell::new(main_volume),
            hitsound_volume: Cell::new(hitsound_volume),
            locked_input: Cell::new(false),
            offset: Cell::new(offset),
            promises: RefCell::new(PromiseExecutor::new()),
            packet_tx,
            game_tx,
            hitsound_track,
            main_track,
            playfield_size: Cell::new(playfield_size),
            max_stack: Cell::new(max_stack),
            mods: RefCell::new(Vec::new()),
            rate: Cell::new(1.0),
            chart_db: RefCell::new(chart_db),
        });

        let azusa = if let Some(token) = token {
            Some(Azusa::new(token).await)
        } else {
            None
        };

        Game {
            _keep,
            screen: if first_time {
                Box::new(SetupScreen::new())
            } else {
                Box::new(SelectScreen::new(data.clone()).await)
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
            hitsound_volume_handle,
            main_volume_handle,
            login_request: None,
            screen_loading_promise: None,
        }
    }

    pub async fn update(&mut self) {
        let time = self.data.state().music.position() as f32;
        let playing = matches!(
            self.data.state().music.state(),
            PlaybackState::Playing | PlaybackState::Stopping | PlaybackState::Pausing
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
                    // The average frames per "audio frame" will appoximately be the average time between audio frames divided by the time between frames.
                    let frames_per_audio_frame = avg_delta / get_frame_time();
                    self.data.state_mut().audio_frame_skip = frames_per_audio_frame as u32;
                }

                self.data
                    .predicted_time
                    .set(self.data.predicted_time.get() + get_frame_time() * self.data.rate.get());
            } else {
                let predicted_time = self.data.predicted_time.get();
                if predicted_time != time {
                    log!(
                        LogType::AudioPerformance,
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
                    log!(LogType::AudioPerformance, "Wow! Perfect!");
                }

                self.audio_deltas.push(delta);
                self.data.predicted_time.set(time);
            }
        }

        if let Some(azusa) = &self.azusa {
            if azusa.connected() && is_key_pressed(KeyCode::F9) {
                if let Some(OverlayEnum::Chat(_)) = self.overlay {
                    log!(LogType::General, "Closing chat overlay");
                    self.overlay = None;
                } else {
                    log!(LogType::General, "Opening chat overlay");
                    self.overlay = Some(OverlayEnum::Chat(overlay::Chat::new()));
                }
            }
        }

        if is_key_pressed(KeyCode::F1) {
            if let Some(OverlayEnum::Settings(_)) = self.overlay {
                log!(LogType::General, "Closing settings overlay");
                self.overlay = None;
            } else {
                log!(LogType::General, "Opening settings overlay");
                self.overlay = Some(OverlayEnum::Settings(overlay::Settings::new(
                    self.data.clone(),
                )));
            }
        }

        if self.data.is_key_pressed(KeyCode::F3) {
            if let Some(OverlayEnum::Mods(_)) = self.overlay {
                log!(LogType::General, "Closing mods overlay");
                self.overlay = None;
            } else {
                log!(LogType::General, "Opening mods overlay");
                self.overlay = Some(OverlayEnum::Mods(overlay::Mods::new(self.data.clone())));
            }
        }

        if is_key_pressed(KeyCode::F7) && self.azusa.is_none() {
            if let Some(OverlayEnum::Login(_)) = self.overlay {
                log!(LogType::General, "Closing login overlay");
                self.overlay = None;
            } else {
                log!(LogType::General, "Opening login overlay");
                self.overlay = Some(OverlayEnum::Login(overlay::Login::new(self.data.clone())));
            }
        }

        if self.data.is_key_pressed(KeyCode::V) {
            self.data
                .broadcast(GameMessage::change_screen(Visualizer::new()));
        }

        if let Some(overlay) = &mut self.overlay {
            overlay.update(self.data.clone());
            self.data.locked_input.set(true);
        }
        self.screen.update(self.data.clone()).await;
        self.data.locked_input.set(false);

        self.data.promises().poll();

        for msg in self.game_rx.drain() {
            match msg {
                GameMessage::ChangeScreen(screen) => self.screen = screen,
                GameMessage::UpdateMusic {
                    mut handle,
                    looping,
                } => {
                    if looping {
                        handle.settings.loop_behavior =
                            Some(kira::LoopBehavior { start_position: 0. });
                    }
                    self.data.state_mut().music.stop(Tween::default()).unwrap();
                    self.data.state_mut().music =
                        self.data.audio.borrow_mut().play(handle).unwrap();
                }
                GameMessage::PauseMusic => {
                    self.data.state_mut().music.pause(Tween::default()).unwrap()
                }
                GameMessage::ResumeMusic => self
                    .data
                    .state_mut()
                    .music
                    .resume(Tween::default())
                    .unwrap(),
                GameMessage::SetMainVolume(volume) => {
                    self.data.main_volume.set(volume);
                    self.main_volume_handle
                        .set_volume(volume as f64, Tween::default())
                        .unwrap();
                    set_value("main_volume", volume);
                }
                GameMessage::SetHitsoundVolume(volume) => {
                    self.data.hitsound_volume.set(volume);
                    self.hitsound_volume_handle
                        .set_volume(volume as f64, Tween::default())
                        .unwrap();
                    set_value("hitsound_volume", volume);
                }
                GameMessage::SetOffset(offset) => {
                    self.data.offset.set(offset);
                    set_value("offset", offset);
                }
                GameMessage::Login { username, password } => {
                    #[derive(serde::Serialize)]
                    struct LoginRequest {
                        username: String,
                        password: String,
                    }

                    self.login_request = Some(
                        quad_net::http_request::RequestBuilder::new("http://127.0.0.1:8080/login")
                            .method(quad_net::http_request::Method::Post)
                            .body(
                                &serde_json::to_string(&LoginRequest { username, password })
                                    .unwrap(),
                            )
                            .header("Content-Type", "application/json")
                            .send(),
                    );
                }
                GameMessage::LoadScreen(fut) => {
                    let data = self.data.clone();
                    let old_loading_promise =
                        self.screen_loading_promise
                            .replace(self.data.promises().spawn(async move {
                                let screen = fut.await;
                                println!("Loaded Screen. Changing..");
                                data.broadcast(GameMessage::ChangeScreen(screen));
                            }));
                    if let Some(old_loading_promise) = old_loading_promise {
                        println!("Cancelled");
                        self.data.promises().cancel(&old_loading_promise);
                    }
                }
                GameMessage::SetMusicRate(rate) => {
                    self.data.rate.set(rate);
                    self.data
                        .state_mut()
                        .music
                        .set_playback_rate(PlaybackRate::Factor(rate as _), Tween::default())
                        .unwrap();
                }
            }
        }

        if let Some(login_request) = &mut self.login_request {
            if let Some(res) = login_request.try_recv() {
                match res {
                    Ok(resp) => {
                        #[derive(Debug, Clone, serde::Deserialize)]
                        struct LoginResponse {
                            token: String,
                        }

                        let resp: LoginResponse = serde_json::from_str(&resp).unwrap();
                        let token_uuid = uuid::Uuid::parse_str(&resp.token).unwrap();
                        set_value("token", token_uuid);
                        self.azusa = Some(Azusa::new(token_uuid).await);
                        self.overlay = None;
                    }
                    Err(err) => {
                        log!(LogType::General, "Failed to request login token. {}", err)
                    }
                }

                self.login_request = None;
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
                    ServerPacket::Connected { version } => {
                        log!(LogType::Network, "Connected to Azusa ({})!", version);
                        azusa.set_connected(true);
                    }
                    ServerPacket::Echo(s) => {
                        log!(LogType::Network, "Azusa says '{}'", s);
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
    }

    pub fn draw(&self) {
        self.screen.draw(self.data.clone());
        if let Some(overlay) = &self.overlay {
            overlay.draw(self.data.clone());
        }
    }
}
