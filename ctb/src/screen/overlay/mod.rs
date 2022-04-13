

use async_trait::async_trait;

use super::{game::SharedGameData};

pub mod chat;

#[async_trait(?Send)]
pub trait Overlay {
    async fn update(&mut self, data: SharedGameData);
    fn draw(&self, data: SharedGameData);
}
