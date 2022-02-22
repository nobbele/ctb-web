use kira::{
    manager::AudioManager,
    sound::{handle::SoundHandle, Sound, SoundSettings},
};
use macroquad::prelude::*;
use std::{
    collections::HashMap,
    future::Future,
    io::{Cursor, Read},
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

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

pub struct Cache<T> {
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
