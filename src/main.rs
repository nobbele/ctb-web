#![feature(once_cell)]
#![allow(clippy::eq_op)]
use async_trait::async_trait;
use kira::{
    instance::{
        handle::InstanceHandle, InstanceSettings, PauseInstanceSettings, ResumeInstanceSettings,
        StopInstanceSettings,
    },
    manager::{AudioManager, AudioManagerSettings},
    sound::{handle::SoundHandle, Sound, SoundSettings},
};
use macroquad::prelude::*;
use num_format::{Locale, ToFormattedString};
use parking_lot::Mutex;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};
use serde::ser::SerializeMap;
use slotmap::SlotMap;
use std::{
    any::Any,
    cell::Cell,
    collections::HashMap,
    future::Future,
    io::{Cursor, Read},
    marker::PhantomData,
    ops::Add,
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

pub fn set_value<T: serde::Serialize>(key: &str, value: T) {
    let value = serde_json::to_value(value).unwrap();
    #[cfg(target_family = "wasm")]
    {
        let storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
        storage.set_item(key, &value.to_string()).unwrap();
    }
    #[cfg(not(target_family = "wasm"))]
    {
        let mut config: HashMap<String, serde_json::Value> = match std::fs::OpenOptions::new()
            .read(true)
            .open("data/config.json")
        {
            Ok(file) => serde_json::from_reader(file).unwrap(),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    HashMap::new()
                } else {
                    panic!("{:?}", e)
                }
            }
        };

        config.insert(key.to_string(), value);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("data/config.json")
            .unwrap();
        serde_json::to_writer(file, &config).unwrap();
    }
}

pub fn get_value<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
    #[cfg(target_family = "wasm")]
    {
        let storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
        let value = storage.get_item(key).unwrap().unwrap();
        serde_json::from_str(&value).unwrap()
    }
    #[cfg(not(target_family = "wasm"))]
    {
        let document = match std::fs::OpenOptions::new()
            .read(true)
            .open("data/config.json")
        {
            Ok(file) => serde_json::from_reader(file).unwrap(),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    serde_json::Value::Object(serde_json::Map::new())
                } else {
                    panic!("{:?}", e)
                }
            }
        };
        serde_json::from_value(document.get(key)?.clone()).ok()
    }
}

#[allow(dead_code)]
fn cache_to_file<R: Read>(key: &str, get: impl Fn() -> R) -> impl Read {
    let path: PathBuf = format!("data/cache/{}", key).into();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    if std::fs::metadata(&path).is_ok() {
        println!("Reading '{}' from cache", key);
        std::fs::OpenOptions::new().read(true).open(path).unwrap()
    } else {
        println!("Caching '{}'", key);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let mut reader = get();
        std::io::copy(&mut reader, &mut file).unwrap();
        std::fs::OpenOptions::new().read(true).open(path).unwrap()
    }
}

struct Cache<T> {
    #[allow(dead_code)]
    base_path: PathBuf,
    cache: parking_lot::Mutex<HashMap<String, Arc<T>>>,
}

impl<T> Cache<T> {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Cache {
            base_path: base_path.into(),
            cache: parking_lot::Mutex::new(HashMap::new()),
        }
    }
    pub async fn get<'a, F: Future<Output = T>>(
        &'a self,
        key: &str,
        get: impl FnOnce() -> F,
    ) -> Arc<T> {
        match self.cache.lock().entry(key.to_owned()) {
            std::collections::hash_map::Entry::Occupied(o) => o.get().clone(),
            std::collections::hash_map::Entry::Vacant(e) => e.insert(Arc::new(get().await)).clone(),
        }
    }
}

struct WaitForBlockingFuture<T, F> {
    done: Arc<AtomicBool>,
    f: Option<F>,
    thread: Option<JoinHandle<T>>,
}

impl<T, F> WaitForBlockingFuture<T, F>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    pub fn new(f: F) -> Self {
        WaitForBlockingFuture {
            done: Arc::new(AtomicBool::new(false)),
            f: Some(f),
            thread: None,
        }
    }
}

impl<T, F> Future for WaitForBlockingFuture<T, F>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    type Output = T;

    fn poll(
        mut self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.thread.is_some() {
            if self.done.load(Ordering::Relaxed) {
                std::task::Poll::Ready(self.thread.take().unwrap().join().unwrap())
            } else {
                std::task::Poll::Pending
            }
        } else {
            let f = self.f.take().unwrap();
            let done = self.done.clone();
            self.thread = Some(std::thread::spawn(move || {
                let v = f();
                done.store(true, Ordering::Relaxed);
                v
            }));
            std::task::Poll::Pending
        }
    }
}

impl<T, F> Unpin for WaitForBlockingFuture<T, F> {}

impl Cache<SoundHandle> {
    pub async fn get_sound(&self, audio: &mut AudioManager, path: &str) -> SoundHandle {
        (*self
            .get(path, || async {
                let sound_data = load_file(path).await.unwrap();
                let sound = WaitForBlockingFuture::new(|| {
                    Sound::from_wav_reader(Cursor::new(sound_data), SoundSettings::default())
                        .unwrap()
                })
                .await;
                audio.add_sound(sound).unwrap()
            })
            .await)
            .clone()
    }
}

impl Cache<Texture2D> {
    pub async fn get_texture(&self, path: &str) -> Texture2D {
        *self
            .get(path, || async { load_texture(path).await.unwrap() })
            .await
    }
}

fn null_waker() -> std::task::Waker {
    let w = ();
    fn _nothing1(_: *const ()) -> std::task::RawWaker {
        panic!()
    }
    fn _nothing2(_: *const ()) {
        panic!()
    }
    fn _nothing3(_: *const ()) {
        panic!()
    }
    fn _nothing4(_: *const ()) {}
    unsafe {
        std::task::Waker::from_raw(std::task::RawWaker::new(
            &w as *const (),
            &std::task::RawWakerVTable::new(_nothing1, _nothing2, _nothing3, _nothing4),
        ))
    }
}

type Fut = dyn Future<Output = Box<dyn Any>>;
enum FutureOrValue {
    Future(Pin<Box<Fut>>),
    Value(Box<dyn Any>),
}

struct PromiseExecutor {
    promises: SlotMap<slotmap::DefaultKey, FutureOrValue>,
}

impl PromiseExecutor {
    pub fn new() -> Self {
        PromiseExecutor {
            promises: SlotMap::new(),
        }
    }

    pub fn spawn<T: Send + 'static, F>(&mut self, fut: impl FnOnce() -> F + 'static) -> Promise<T>
    where
        F: Future<Output = T> + 'static,
    {
        let key = self.promises.insert(FutureOrValue::Future(Box::pin(async {
            Box::new(fut().await) as Box<dyn Any>
        })));
        Promise {
            cancelled_or_finished: Cell::new(true),
            id: key,
            _phantom: PhantomData,
        }
    }

    pub fn try_get<T: 'static>(&mut self, promise: &Promise<T>) -> Option<T> {
        match &self.promises[promise.id] {
            FutureOrValue::Future(_) => None,
            FutureOrValue::Value(_) => {
                promise.cancelled_or_finished.set(true);
                match self.promises.remove(promise.id).unwrap() {
                    FutureOrValue::Future(_) => unreachable!(),
                    FutureOrValue::Value(v) => Some(*v.downcast().unwrap()),
                }
            }
        }
    }

    pub fn poll(&mut self) {
        let keys = self.promises.keys().collect::<Vec<_>>();
        for key in keys {
            let promise = &mut self.promises[key];
            match promise {
                FutureOrValue::Future(f) => {
                    let waker = null_waker();
                    let mut cx = std::task::Context::from_waker(&waker);
                    match std::future::Future::poll(f.as_mut(), &mut cx) {
                        std::task::Poll::Ready(v) => {
                            self.promises[key] = FutureOrValue::Value(v);
                        }
                        std::task::Poll::Pending => {}
                    }
                }
                FutureOrValue::Value(_) => {}
            }
        }
    }

    pub fn cancel<T>(&mut self, promise: &Promise<T>) {
        promise.cancelled_or_finished.set(true);
        self.promises.remove(promise.id);
    }
}

struct Promise<T> {
    cancelled_or_finished: Cell<bool>,
    id: slotmap::DefaultKey,
    _phantom: PhantomData<T>,
}

impl<T> Drop for Promise<T> {
    fn drop(&mut self) {
        if !self.cancelled_or_finished.get() {
            panic!("Promise droppped without being cancelled or completed!");
        }
    }
}

#[test]
fn test_promise() {
    let mut exec = PromiseExecutor::new();
    let promise = exec.spawn(|| async { std::future::ready(5).await });
    assert_eq!(exec.try_get(&promise), None);
    exec.poll();
    assert_eq!(exec.try_get(&promise), Some(5));

    struct ExampleFuture {
        done: Arc<std::sync::atomic::AtomicBool>,
    }

    impl ExampleFuture {
        fn new() -> Self {
            let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let done_clone = done.clone();
            let _ = std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(50));
                done_clone.store(true, std::sync::atomic::Ordering::Relaxed);
            });
            ExampleFuture { done }
        }
    }

    impl Future for ExampleFuture {
        type Output = ();

        fn poll(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            if self.done.load(std::sync::atomic::Ordering::Relaxed) {
                std::task::Poll::Ready(())
            } else {
                std::task::Poll::Pending
            }
        }
    }

    let promise = exec.spawn(|| async { ExampleFuture::new().await });

    loop {
        exec.poll();

        if let Some(_) = exec.try_get(&promise) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

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
                let dist = (next_fruit.position - fruit.position).abs();
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

    /// This needs to be tracked separately due to floating point imprecision.
    internal_score: f32,
    chain_miss_count: u32,

    /// Max = 1,000,000
    score: u32,
    /// [0, 1]
    accuracy: f32,
    /// [0, 1]
    hp: f32,
}

fn polynomial(x: f32, coeffs: &[f32]) -> f32 {
    coeffs.iter().rev().fold(0., |acc, &c| acc * x + c)
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
            chain_miss_count: 0,
            score: 0,
            accuracy: 1.0,
            hp: 1.0,
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
            self.chain_miss_count = 0;

            self.hp += (self.combo as f32 / self.max_combo as f32) * 0.1;
            self.hp = self.hp.min(1.0);
        } else {
            self.combo = 0;
            self.miss_count += 1;

            #[allow(clippy::excessive_precision)]
            let hp_drain = polynomial(
                self.chain_miss_count as f32,
                &[
                    1.0029920966561545e+000,
                    7.4349034374388925e+000,
                    -9.1951466248253642e+000,
                    4.8111412580746844e+000,
                    -1.2397067078689683e+000,
                    1.7714300116489434e-001,
                    -1.4390229652509492e-002,
                    6.2392424752562498e-004,
                    -1.1231385529709802e-005,
                ],
            ) / 40.;
            dbg!(self.chain_miss_count);
            println!("{}%", hp_drain * 100.);
            self.hp -= hp_drain;
            self.hp = self.hp.max(0.);

            self.chain_miss_count += 1;
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

#[test]
fn test_hp() {
    let mut recorder = ScoreRecorder::new(100);
    assert_eq!(recorder.hp, 1.0);
    for _ in 0..10 {
        recorder.register_judgement(true);
    }
    assert_eq!(recorder.hp, 1.0);
    recorder.register_judgement(false);
    assert_eq!(recorder.hp, 0.9749252);
    for _ in 0..10 {
        recorder.register_judgement(true);
    }
    assert_eq!(recorder.hp, 1.0);
    for _ in 0..3 {
        recorder.register_judgement(false);
    }
    assert_eq!(recorder.hp, 0.8362208);
    recorder.register_judgement(true);
    for _ in 0..6 {
        recorder.register_judgement(false);
    }
    assert_eq!(recorder.hp, 0.22481588);
    recorder.register_judgement(true);
    for _ in 0..12 {
        recorder.register_judgement(false);
    }
    assert_eq!(recorder.hp, 0.0);
}

struct Gameplay {
    recorder: ScoreRecorder,

    time: f32,
    predicted_time: f32,
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

        let mut sound = data
            .audio_cache
            .get_sound(
                &mut *data.audio.lock(),
                &format!("resources/{}/audio.wav", map),
            )
            .await;

        data.state
            .lock()
            .music
            .stop(StopInstanceSettings::new())
            .unwrap();
        data.state.lock().music = sound.play(InstanceSettings::default().volume(0.5)).unwrap();
        data.state
            .lock()
            .music
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
            time: -time_countdown,
            predicted_time: -time_countdown,
            prev_time: -time_countdown,
            recorder: ScoreRecorder::new(chart.fruits.len() as u32),
            position: 256.,
            hyper_multiplier: 1.,
            deref_delete: Vec::new(),
            queued_fruits: (0..chart.fruits.len()).collect(),
            chart,
            show_debug_hitbox: false,
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

    pub fn catcher_speed(&self, dash: KeyCode) -> f32 {
        catcher_speed(is_key_down(dash), self.hyper_multiplier)
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

    pub fn movement_direction(&self, left: KeyCode, right: KeyCode) -> i32 {
        is_key_down(right) as i32 - is_key_down(left) as i32
    }
}

#[async_trait(?Send)]
impl Screen for Gameplay {
    async fn update(&mut self, data: Arc<GameData>) {
        let binds = data.state.lock().binds;
        let catcher_y = self.catcher_y();

        if !self.started {
            self.prev_time = self.time;
            self.time = -self.time_countdown;
            if self.time_countdown > 0. {
                self.time_countdown -= get_frame_time();
            } else {
                data.state
                    .lock()
                    .music
                    .resume(ResumeInstanceSettings::new())
                    .unwrap();
                self.started = true;
            }
        } else {
            self.prev_time = self.time;
            self.time = data.state.lock().music.position() as f32;
        }

        if self.time - self.prev_time == 0. {
            let audio_frame_skip = data.state.lock().audio_frame_skip;
            self.predicted_time += get_frame_time();
            if audio_frame_skip != 0 {
                self.predicted_time -= get_frame_time() / audio_frame_skip as f32;
            }
        } else {
            // Print prediction error
            /*let audio_frame_skip = data.audio_frame_skip.get();
            let prediction_delta = self.time - self.predicted_time;
            if audio_frame_skip != 0 {
                let audio_frame_time = get_frame_time() * audio_frame_skip as f32;
                let prediction_off = prediction_delta / audio_frame_time;
                println!("Off by {:.2}%", prediction_off);
            }
            if prediction_delta < 0. {
                println!(
                    "Overcompensated by {}ms",
                    (-prediction_delta * 1000.).round() as i32
                );
            }*/
            self.predicted_time = self.time;
        }

        self.position = self
            .position
            .add(
                self.movement_direction(binds.left, binds.right) as f32
                    * self.catcher_speed(binds.dash)
                    * get_frame_time(),
            )
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

        if self.recorder.hp == 0. || self.queued_fruits.is_empty() {
            data.state.lock().queued_screen = Some(Box::new(ResultScreen {
                title: "TODO".to_string(),
                difficulty: "TODO".to_string(),
                score: self.recorder.score,
                hit_count: self.recorder.hit_count,
                miss_count: self.recorder.miss_count,
                top_combo: self.recorder.top_combo,
                accuracy: self.recorder.accuracy,
            }));
        }

        if is_key_pressed(KeyCode::O) {
            self.show_debug_hitbox = !self.show_debug_hitbox;
        }
        if is_key_pressed(KeyCode::Escape) {
            data.state
                .lock()
                .music
                .stop(StopInstanceSettings::new())
                .unwrap();
            data.state.lock().queued_screen = Some(Box::new(MainMenu::new(data.clone())));
        }
    }

    fn draw(&self, data: Arc<GameData>) {
        if let Some(background) = data.state.lock().background {
            draw_texture_ex(
                background,
                0.,
                0.,
                Color::new(1., 1., 1., 0.2),
                DrawTextureParams {
                    dest_size: Some(vec2(screen_width(), screen_height())),
                    ..Default::default()
                },
            );
        }
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
                self.fruit_y(self.predicted_time, fruit.time)
                    - self.chart.fruit_radius * self.scale(),
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

        let text = format!("{}%", self.recorder.hp * 100.);
        let text_dim = measure_text(&text, None, 36, 1.0);
        draw_text(
            &text,
            screen_width() / 2. - text_dim.width / 2.,
            23.,
            36.,
            WHITE,
        );
    }
}

struct ResultScreen {
    title: String,
    difficulty: String,

    score: u32,
    hit_count: u32,
    miss_count: u32,
    top_combo: u32,
    accuracy: f32,
}

#[async_trait(?Send)]
impl Screen for ResultScreen {
    fn draw(&self, _data: Arc<GameData>) {
        draw_text(
            &self.title,
            screen_width() / 2.,
            screen_height() / 2. - 100.,
            36.,
            WHITE,
        );
        draw_text(
            &self.difficulty,
            screen_width() / 2.,
            screen_height() / 2. - 50.,
            36.,
            WHITE,
        );
        draw_text(
            &format!("{}x", self.top_combo),
            screen_width() / 2.,
            screen_height() / 2. - 10.,
            36.,
            WHITE,
        );
        draw_text(
            &format!("{}/{}", self.hit_count, self.miss_count),
            screen_width() / 2.,
            screen_height() / 2. + 30.,
            36.,
            WHITE,
        );
        draw_text(
            &format!("{}", self.score),
            screen_width() / 2.,
            screen_height() / 2. + 70.,
            36.,
            WHITE,
        );
        draw_text(
            &format!("{:.2}%", self.accuracy * 100.),
            screen_width() / 2.,
            screen_height() / 2. + 110.,
            36.,
            WHITE,
        );
    }

    async fn update(&mut self, data: Arc<GameData>) {
        if is_key_pressed(KeyCode::Escape) {
            data.state.lock().queued_screen = Some(Box::new(MainMenu::new(data.clone())));
        }
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

// Implementors assumed to call set_bounds in its new() method.
// Implementors assumed propogate draw_bounds to children.
trait UiElement {
    fn draw(&self, data: Arc<GameData>);
    fn draw_bounds(&self) {
        let bounds = self.bounds();
        draw_rectangle(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            Color::new(1.0, 0.0, 0.0, 0.5),
        );
    }

    fn set_bounds(&mut self, rect: Rect);
    fn bounds(&self) -> Rect;
    fn refresh_bounds(&mut self) {
        self.set_bounds(self.bounds());
    }

    fn update(&mut self, data: Arc<GameData>);
    fn handle_message(&mut self, _message: &Message) {}
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Popout {
    None,
    Left,
    Right,
    Towards,
}

struct MenuButton {
    id: String,
    title: String,
    rect: Rect,
    visible_rect: Rect,
    tx: flume::Sender<Message>,
    hovered: bool,
    selected: bool,
    offset: f32,
    popout: Popout,
}
const SELECTED_COLOR: Color = Color::new(1.0, 1.0, 1.0, 1.0);
const HOVERED_COLOR: Color = Color::new(0.5, 0.5, 0.8, 1.0);
const IDLE_COLOR: Color = Color::new(0.5, 0.5, 0.5, 1.0);

impl MenuButton {
    pub fn new(
        id: String,
        title: String,
        popout: Popout,
        rect: Rect,
        tx: flume::Sender<Message>,
    ) -> Self {
        let mut button = MenuButton {
            id,
            title,
            rect: Rect::default(),
            visible_rect: Rect::default(),
            tx,
            hovered: false,
            selected: false,
            offset: 0.,
            popout,
        };
        button.set_bounds(rect);
        button
    }
}

impl UiElement for MenuButton {
    fn draw(&self, data: Arc<GameData>) {
        draw_texture_ex(
            data.button,
            self.visible_rect.x,
            self.visible_rect.y,
            if self.selected {
                SELECTED_COLOR
            } else if self.hovered {
                HOVERED_COLOR
            } else {
                IDLE_COLOR
            },
            DrawTextureParams {
                dest_size: Some(vec2(self.visible_rect.w, self.visible_rect.h)),
                ..Default::default()
            },
        );
        let title_length = measure_text(&self.title, None, 36, 1.);
        draw_text(
            &self.title,
            self.visible_rect.x + self.visible_rect.w / 2. - title_length.width / 2.,
            self.visible_rect.y + self.visible_rect.h / 2. - title_length.height / 2.,
            36.,
            WHITE,
        )
    }

    fn update(&mut self, _data: Arc<GameData>) {
        if self.visible_rect.contains(mouse_position().into()) {
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
        } else if self.hovered {
            self.tx
                .send(Message {
                    sender: self.id.clone(),
                    data: MessageData::MenuButton(MenuButtonMessage::Unhovered),
                })
                .unwrap();
        }

        if self.selected {
            self.offset += 2. * get_frame_time();
        } else if self.hovered {
            if self.offset <= 0.8 {
                self.offset += 2. * get_frame_time();
                self.offset = self.offset.min(0.8);
            }
        } else {
            self.offset -= 2. * get_frame_time();
        }
        self.offset = self.offset.clamp(0., 1.0);
        let x_offset = self.offset * self.rect.w / 4.;
        let y_offset = self.offset * self.rect.h / 4.;

        match self.popout {
            Popout::None => {}
            Popout::Left => {
                self.visible_rect.x = self.rect.x - x_offset;
            }
            Popout::Right => {
                self.visible_rect.x = self.rect.x + x_offset;
            }
            Popout::Towards => {
                self.visible_rect.w = self.rect.w + x_offset;
                self.visible_rect.h = self.rect.h + y_offset;
                self.visible_rect.x = self.rect.x - x_offset / 2.;
                self.visible_rect.y = self.rect.y - y_offset / 2.;
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

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
        self.visible_rect = rect;
    }

    fn bounds(&self) -> Rect {
        self.rect
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
    rect: Rect,
}

impl MenuButtonList {
    pub fn new(
        id: String,
        popout: Popout,
        rect: Rect,
        titles: &[&str],
        tx: flume::Sender<Message>,
    ) -> Self {
        let mut list = MenuButtonList {
            id: id.clone(),
            buttons: titles
                .iter()
                .enumerate()
                .map(|(idx, &title)| {
                    MenuButton::new(
                        format!("{}-{}", &id, idx),
                        title.to_owned(),
                        popout,
                        Rect::default(),
                        tx.clone(),
                    )
                })
                .collect(),
            selected: 0,
            tx,
            rect: Rect::default(),
        };
        list.set_bounds(rect);
        list
    }
}

impl UiElement for MenuButtonList {
    fn draw(&self, data: Arc<GameData>) {
        for button in &self.buttons {
            button.draw(data.clone());
        }
    }

    fn update(&mut self, data: Arc<GameData>) {
        for button in &mut self.buttons {
            button.update(data.clone());
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

    fn set_bounds(&mut self, rect: Rect) {
        //let button_height = rect.h / self.buttons.len() as f32;
        for (idx, button) in self.buttons.iter_mut().enumerate() {
            button.set_bounds(Rect::new(
                rect.x,
                rect.y + (100. + 5.) * idx as f32,
                rect.w,
                100.,
            ));
        }
        self.rect = rect;
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn draw_bounds(&self) {
        let bounds = self.bounds();
        draw_rectangle(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            Color::new(0.0, 0.0, 0.5, 0.5),
        );
        for button in &self.buttons {
            button.draw_bounds();
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
    loading_promise: Option<Promise<(SoundHandle, Texture2D)>>,
}

impl MainMenu {
    pub fn new(_data: Arc<GameData>) -> Self {
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
            MapListing {
                title: "Troublemaker".to_string(),
                difficulties: vec![
                    "Cup".to_string(),
                    "Equim's Rain".to_string(),
                    "Kagari's Himedose".to_string(),
                    "MBomb's Light Rain".to_string(),
                    "Platter".to_string(),
                    "tocean's Salad".to_string(),
                ],
            },
        ];
        let map_list = MenuButtonList::new(
            "button_list".to_string(),
            Popout::Right,
            Rect::new(-400. / 4., 0., 400., 400.),
            maps.iter()
                .map(|map| map.title.as_str())
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
                Popout::None,
                Rect::new(
                    screen_width() / 2. - 400. / 2.,
                    screen_height() - 100.,
                    400.,
                    100.,
                ),
                tx,
            ),
            loading_promise: None,
        }
    }
}

#[async_trait(?Send)]
impl Screen for MainMenu {
    async fn update(&mut self, data: Arc<GameData>) {
        if self.selected_map != self.prev_selected_map {
            let data_clone = data.clone();
            let map_title = self.maps[self.selected_map].title.clone();
            if let Some(loading_promise) = &self.loading_promise {
                data.exec.lock().cancel(loading_promise);
            }
            self.loading_promise = Some(data.exec.lock().spawn(move || async move {
                let sound = data_clone
                    .audio_cache
                    .get_sound(
                        &mut data_clone.audio.lock(),
                        &format!("resources/{}/audio.wav", map_title),
                    )
                    .await;
                let background = data_clone
                    .image_cache
                    .get_texture(&format!("resources/{}/bg.png", map_title))
                    .await;
                (sound, background)
            }));

            self.prev_selected_map = self.selected_map;
        }

        if let Some(loading_promise) = &self.loading_promise {
            if let Some((mut sound, background)) = data.exec.lock().try_get(loading_promise) {
                data.state
                    .lock()
                    .music
                    .stop(StopInstanceSettings::new())
                    .unwrap();
                data.state.lock().background = Some(background);
                data.state.lock().music =
                    sound.play(InstanceSettings::default().volume(0.5)).unwrap();

                let difficulty_list = MenuButtonList::new(
                    "difficulty_list".to_string(),
                    Popout::Left,
                    Rect::new(screen_width() - 400. + 400. / 4., 0., 400., 400.),
                    &self.maps[self.selected_map]
                        .difficulties
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>(),
                    self.tx.clone(),
                );
                self.tx
                    .send(Message {
                        sender: difficulty_list.id.clone(),
                        data: MessageData::MenuButtonList(MenuButtonListMessage::Click(0)),
                    })
                    .unwrap();
                self.difficulty_list = Some(difficulty_list);

                self.loading_promise = None;
            }
        }

        if let Some(difficulty_list) = &self.difficulty_list {
            if is_key_pressed(KeyCode::Down) {
                self.tx
                    .send(Message {
                        sender: difficulty_list.id.clone(),
                        data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                            (difficulty_list.selected + 1) % difficulty_list.buttons.len(),
                        )),
                    })
                    .unwrap();
            } else if is_key_pressed(KeyCode::Up) {
                self.tx
                    .send(Message {
                        sender: difficulty_list.id.clone(),
                        data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                            (difficulty_list.selected + difficulty_list.buttons.len() - 1)
                                % difficulty_list.buttons.len(),
                        )),
                    })
                    .unwrap();
            }
        }

        if is_key_pressed(KeyCode::Right) {
            self.tx
                .send(Message {
                    sender: self.map_list.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                        (self.map_list.selected + 1) % self.map_list.buttons.len(),
                    )),
                })
                .unwrap();
        } else if is_key_pressed(KeyCode::Left) {
            self.tx
                .send(Message {
                    sender: self.map_list.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                        (self.map_list.selected + self.map_list.buttons.len() - 1)
                            % self.map_list.buttons.len(),
                    )),
                })
                .unwrap();
        }

        if is_key_pressed(KeyCode::Enter) {
            self.tx
                .send(Message {
                    sender: self.start.id.clone(),
                    data: MessageData::MenuButton(MenuButtonMessage::Selected),
                })
                .unwrap();
        }

        for message in self.rx.drain() {
            self.map_list.handle_message(&message);
            if let Some(difficulty_list) = &mut self.difficulty_list {
                difficulty_list.handle_message(&message);
            }
            self.start.handle_message(&message);
            if message.sender == self.map_list.id {
                if let MessageData::MenuButtonList(MenuButtonListMessage::Selected(idx)) =
                    message.data
                {
                    self.selected_map = idx;
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
                    data.state.lock().queued_screen = Some(Box::new(
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
        if let Some(difficulty_list) = &mut self.difficulty_list {
            difficulty_list.update(data.clone());
        }
        self.start.update(data.clone());
    }

    fn draw(&self, data: Arc<GameData>) {
        if let Some(background) = data.state.lock().background {
            draw_texture_ex(
                background,
                0.,
                0.,
                Color::new(1., 1., 1., 0.6),
                DrawTextureParams {
                    dest_size: Some(vec2(screen_width(), screen_height())),
                    ..Default::default()
                },
            );
        }
        self.map_list.draw(data.clone());
        if let Some(ref list) = self.difficulty_list {
            list.draw(data.clone());
        }
        self.start.draw(data);

        if self.loading_promise.is_some() {
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

#[derive(Debug, Clone, Copy)]
struct KeyBinds {
    right: KeyCode,
    left: KeyCode,
    dash: KeyCode,
}

impl serde::Serialize for KeyBinds {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("left", &(self.left as u32))?;
        map.serialize_entry("right", &(self.right as u32))?;
        map.serialize_entry("dash", &(self.dash as u32))?;
        map.end()
    }
}

struct KeyBindVisitor;

impl<'de> serde::de::Visitor<'de> for KeyBindVisitor {
    type Value = KeyBinds;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("Expected a map with the keys 'left', 'right', and 'dash'")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut binds = KeyBinds {
            right: KeyCode::D,
            left: KeyCode::A,
            dash: KeyCode::RightShift,
        };
        while let Some((key, value)) = map.next_entry::<_, u32>()? {
            match key {
                "left" => {
                    binds.left = KeyCode::from(value);
                }
                "right" => {
                    binds.right = KeyCode::from(value);
                }
                "dash" => {
                    binds.dash = KeyCode::from(value);
                }
                _ => panic!(),
            }
        }
        Ok(binds)
    }
}

impl<'de> serde::Deserialize<'de> for KeyBinds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(KeyBindVisitor)
    }
}

struct Setup {
    binding_types: MenuButtonList,
    rx: flume::Receiver<Message>,
}

impl Setup {
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();
        Setup {
            binding_types: MenuButtonList::new(
                "binding_types".to_string(),
                Popout::Towards,
                Rect::new(
                    screen_width() / 2. - 400. / 2.,
                    screen_height() / 2. - 105. * 3. / 2.,
                    400.,
                    400.,
                ),
                &[
                    "Left-handed (A D RShift)",
                    "Right-handed (Left Right LShift)",
                    "Custom (TODO)",
                ],
                tx,
            ),
            rx,
        }
    }
}

#[async_trait(?Send)]
impl Screen for Setup {
    fn draw(&self, data: Arc<GameData>) {
        self.binding_types.draw(data);
    }

    async fn update(&mut self, data: Arc<GameData>) {
        self.binding_types.update(data.clone());
        for message in self.rx.drain() {
            self.binding_types.handle_message(&message);
            if message.sender == self.binding_types.id {
                if let MessageData::MenuButtonList(MenuButtonListMessage::Selected(idx)) =
                    message.data
                {
                    let key_binds = match idx {
                        0 => KeyBinds {
                            right: KeyCode::D,
                            left: KeyCode::A,
                            dash: KeyCode::RightShift,
                        },
                        1 => KeyBinds {
                            right: KeyCode::Right,
                            left: KeyCode::Left,
                            dash: KeyCode::LeftShift,
                        },
                        _ => todo!(),
                    };
                    set_value("first_time", false);
                    set_value("binds", key_binds);
                    data.state.lock().binds = key_binds;
                    data.state
                        .lock()
                        .queued_screen
                        .replace(Box::new(MainMenu::new(data.clone())));
                }
            }
        }
    }
}

struct GameState {
    music: InstanceHandle,
    background: Option<Texture2D>,
    queued_screen: Option<Box<dyn Screen>>,
    audio_frame_skip: u32,
    binds: KeyBinds,
}

struct GameData {
    audio: Mutex<AudioManager>,
    catcher: Texture2D,
    fruit: Texture2D,
    button: Texture2D,

    audio_cache: Cache<SoundHandle>,
    image_cache: Cache<Texture2D>,

    state: Mutex<GameState>,
    exec: Arc<Mutex<PromiseExecutor>>,
}

struct Game {
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
                Box::new(Setup::new())
            } else {
                Box::new(MainMenu::new(data.clone()))
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

#[macroquad::main(window_conf)]
async fn main() {
    let exec = Arc::new(Mutex::new(PromiseExecutor::new()));
    let mut game = Game::new(exec.clone()).await;
    loop {
        exec.lock().poll();
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
