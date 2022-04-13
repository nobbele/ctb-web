use super::game::SharedGameData;
use async_trait::async_trait;

mod chat;
mod settings;

pub use chat::Chat;
pub use settings::Settings;

#[async_trait(?Send)]
pub trait Overlay {
    async fn update(&mut self, data: SharedGameData);
    fn draw(&self, data: SharedGameData);
}

pub enum OverlayEnum {
    Chat(Chat),
    Settings(Settings),
}

#[async_trait(?Send)]
impl Overlay for OverlayEnum {
    async fn update(&mut self, data: SharedGameData) {
        match self {
            OverlayEnum::Chat(c) => c.update(data).await,
            OverlayEnum::Settings(s) => s.update(data).await,
        }
    }

    fn draw(&self, data: SharedGameData) {
        match self {
            OverlayEnum::Chat(c) => c.draw(data),
            OverlayEnum::Settings(s) => s.draw(data),
        }
    }
}
