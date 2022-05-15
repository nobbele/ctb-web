use kira::{
    sound::static_sound::{StaticSoundData, StaticSoundSettings},
    track::TrackId,
};
use macroquad::prelude::*;
use std::{
    cell::RefCell,
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
        std::fs::OpenOptions::new().read(true).open(path).unwrap()
    } else {
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

#[derive(Debug)]
pub enum LoadError {
    NotWhitelisted,
    Generic,
}

pub struct Cache<T> {
    #[allow(dead_code)]
    base_path: PathBuf,
    cache: RefCell<HashMap<String, Arc<T>>>,
    whitelist: RefCell<Vec<String>>,
}

impl<T> Cache<T> {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Cache {
            base_path: base_path.into(),
            cache: RefCell::new(HashMap::new()),
            whitelist: RefCell::new(Vec::new()),
        }
    }
    pub async fn get<F: Future<Output = Result<T, LoadError>>>(
        &self,
        key: &str,
        get: impl FnOnce() -> F,
    ) -> Result<Arc<T>, LoadError> {
        Ok(match self.cache.borrow_mut().entry(key.to_owned()) {
            std::collections::hash_map::Entry::Occupied(o) => o.get().clone(),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(Arc::new(get().await?)).clone()
            }
        })
    }

    pub fn whitelist(&self, path: String) {
        self.whitelist.borrow_mut().push(path);
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
            #[cfg(target_family = "wasm")]
            {
                self.done.store(true, Ordering::Relaxed);
                std::task::Poll::Ready((self.f.take().unwrap())())
            }
            #[cfg(not(target_family = "wasm"))]
            {
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
}

impl<T, F> Unpin for WaitForBlockingFuture<T, F> {}

impl Cache<StaticSoundData> {
    pub async fn get_sound_bypass(
        &self,
        path: &str,
        track: TrackId,
    ) -> Result<StaticSoundData, LoadError> {
        let res = self
            .get(path, move || async move {
                let sound_data = load_file(path).await.map_err(|_| LoadError::Generic)?;
                WaitForBlockingFuture::new(move || {
                    Ok(StaticSoundData::from_cursor(
                        Cursor::new(sound_data),
                        StaticSoundSettings::default().track(track),
                    )
                    .unwrap())
                })
                .await
            })
            .await;
        Ok((*res?).clone())
    }
    pub async fn get_sound(
        &self,
        path: &str,
        track: TrackId,
    ) -> Result<StaticSoundData, LoadError> {
        if !self.whitelist.borrow().iter().any(|s| s == path) {
            return Err(LoadError::NotWhitelisted);
        }

        self.get_sound_bypass(path, track).await
    }
}

impl Cache<Texture2D> {
    pub async fn get_texture(&self, path: &str) -> Texture2D {
        let res = self
            .get(path, || async { Ok(load_texture(path).await.unwrap()) })
            .await;
        *res.unwrap()
    }
}
