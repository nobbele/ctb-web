use client::screen::game::Game;
pub use client::*;
use macroquad::prelude::*;

//  !!  Online Map Selection
// TODO Optimize menu button list (can we query map data from a local database rather than in-memory vecs?)
// TODO and split added maps menu and online listing
// TODO and implement map listings in the API (with basic info used for previewing SR, title, diffs, etc)
// TODO and load beatmaps partially and stream the previews from web (loading and non-blocking)
// TODO and implement pagination to the online map select (I think, no idea how this selection menu will look)

//  !!  Fix UI in select screen..
// TODO Preview profile image

//  !!  Finish website
// TODO Remove debug stuff, add profile page

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
