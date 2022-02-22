#![allow(clippy::eq_op)]
use macroquad::prelude::*;
use parking_lot::Mutex;
use promise::PromiseExecutor;
use screen::{Game, GameData};
use std::sync::Arc;

pub mod cache;
pub mod chart;
pub mod config;
pub mod promise;
pub mod score_recorder;
pub mod screen;
pub mod ui;

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
