use std::sync::Arc;

use async_trait::async_trait;

use super::GameData;

pub mod chat;

#[async_trait(?Send)]
pub trait Overlay {
    async fn update(&mut self, data: Arc<GameData>);
    fn draw(&self, data: Arc<GameData>);
}
