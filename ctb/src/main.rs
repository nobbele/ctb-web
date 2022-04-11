use ctb::screen::game::Game;
pub use ctb::*;
use macroquad::prelude::*;
use parking_lot::Mutex;
use promise::PromiseExecutor;

#[macroquad::main(window_conf)]
async fn main() {
    println!("Starting game..");
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