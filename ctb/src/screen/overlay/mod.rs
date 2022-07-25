use super::game::SharedGameData;
use async_trait::async_trait;

mod chat;
mod login;
mod mods;
mod settings;

pub use self::login::Login;
pub use chat::Chat;
pub use mods::Mods;
pub use settings::Settings;

#[async_trait(?Send)]
pub trait Overlay {
    fn update(&mut self, data: SharedGameData);
    fn draw(&self, data: SharedGameData);
}

pub enum OverlayEnum {
    Chat(Chat),
    Settings(Settings),
    Login(Login),
    Mods(Mods),
}

#[async_trait(?Send)]
impl Overlay for OverlayEnum {
    fn update(&mut self, data: SharedGameData) {
        match self {
            OverlayEnum::Chat(c) => c.update(data),
            OverlayEnum::Settings(s) => s.update(data),
            OverlayEnum::Login(l) => l.update(data),
            OverlayEnum::Mods(m) => m.update(data),
        }
    }

    fn draw(&self, data: SharedGameData) {
        match self {
            OverlayEnum::Chat(c) => c.draw(data),
            OverlayEnum::Settings(s) => s.draw(data),
            OverlayEnum::Login(l) => l.draw(data),
            OverlayEnum::Mods(m) => m.draw(data),
        }
    }
}
