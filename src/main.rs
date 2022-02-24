#![allow(clippy::eq_op)]
use macroquad::prelude::*;
use parking_lot::Mutex;
use promise::PromiseExecutor;
use screen::{Game, GameData};

pub mod cache;
pub mod chart;
pub mod config;
pub mod leaderboard;
pub mod promise;
pub mod score;
pub mod screen;
pub mod ui;

#[macroquad::main(window_conf)]
async fn main() {
    let exec = Mutex::new(PromiseExecutor::new());
    let mut game = Game::new(exec).await;
    loop {
        game.data.exec.lock().poll();
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
