use ctb::screen::game::Game;
pub use ctb::*;
use macroquad::prelude::*;

#[macroquad::main(window_conf)]
async fn main() {
    println!("Starting game..");
    let mut game = Game::new().await;
    loop {
        // TODO We definitely need to move this to fixed delta for determinism (esp. with replays and potentionally multiplayer).
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
