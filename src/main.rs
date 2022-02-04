use std::{io::Cursor, ops::Add};

use kira::{
    instance::{handle::InstanceHandle, InstanceSettings},
    manager::{AudioManager, AudioManagerSettings},
    sound::{Sound, SoundSettings},
};
use macroquad::prelude::*;

pub fn can_catch_fruit(catcher_hitbox: Rect, fruit_hitbox: Rect) -> bool {
    catcher_hitbox.intersect(fruit_hitbox).is_some()
}

pub fn catcher_speed(dashing: bool, hyper_multiplier: f32) -> f32 {
    let mut mov_speed = 500.;
    if dashing {
        mov_speed *= 2. * hyper_multiplier;
    }
    mov_speed
}

#[derive(Debug)]
struct Fruit {
    position: f32,
    time: f32,
    hyper: Option<f32>,
}

impl Fruit {
    pub fn from_hitobject(hitobject: &osu_types::HitObject) -> Self {
        Fruit {
            position: hitobject.position.0 as f32,
            time: hitobject.time as f32 / 1000.,
            hyper: None,
        }
    }
}

struct Game {
    #[allow(dead_code)]
    audio: AudioManager,
    catcher: Texture2D,
    fruit: Texture2D,

    handle: InstanceHandle,

    time: f32,
    prev_time: f32,
    position: f32,
    hyper_multiplier: f32,

    combo: u32,

    fruits: Vec<Fruit>,
    deref_delete: Vec<usize>,
}

//const FIXED_DELTA: f32 = 1. / 60.;

impl Game {
    pub async fn new() -> Self {
        let beatmap_data = load_file("resources/aru - Kizuato (Benny-) [Platter].osu")
            .await
            .unwrap();
        let beatmap_content = std::str::from_utf8(&beatmap_data).unwrap();
        let beatmap =
            osu_parser::load_content(beatmap_content, osu_parser::BeatmapParseOptions::default())
                .unwrap();
        let mut fruits = Vec::with_capacity(beatmap.hit_objects.len());
        for (idx, hitobject) in beatmap.hit_objects.iter().enumerate() {
            let mut fruit = Fruit::from_hitobject(hitobject);

            // If you can't get to the fruit center in time, we need to give the player some extra speed.
            // TODO use same implementation as osu!catch.
            if let Some(next_hitobject) = beatmap.hit_objects.get(idx + 1) {
                let next_fruit = Fruit::from_hitobject(next_hitobject);
                let dist = next_fruit.position - fruit.position;
                let time = next_fruit.time - fruit.time;
                let required_time = dist / catcher_speed(true, 1.);
                if required_time > time {
                    fruit.hyper = Some(required_time / time);
                };
            }

            fruits.push(fruit);
        }

        let mut audio = AudioManager::new(AudioManagerSettings::default()).unwrap();

        let song = load_file("resources/audio.wav").await.unwrap();
        let sound_data =
            Sound::from_wav_reader(Cursor::new(song), SoundSettings::default()).unwrap();
        let mut sound = audio.add_sound(sound_data).unwrap();
        let instance = sound.play(InstanceSettings::default().volume(0.5)).unwrap();

        Game {
            audio,
            catcher: load_texture("resources/catcher.png").await.unwrap(),
            fruit: load_texture("resources/fruit.png").await.unwrap(),
            time: instance.position() as f32,
            prev_time: instance.position() as f32,
            handle: instance,
            position: 256.,
            hyper_multiplier: 1.,
            combo: 0,
            deref_delete: Vec::with_capacity(fruits.len()),
            fruits,
        }
    }

    pub fn catcher_y(&self) -> f32 {
        screen_height() - 102.
    }

    pub fn fruit_y(&self, time: f32, target: f32) -> f32 {
        let time_left = target - time;
        self.catcher_y() - time_left * 600.
    }

    pub fn catcher_speed(&self) -> f32 {
        catcher_speed(is_key_down(KeyCode::RightShift), self.hyper_multiplier)
    }

    pub fn playfield_to_screen_x(&self, x: f32) -> f32 {
        self.playfield_x() + x
    }

    pub fn playfield_x(&self) -> f32 {
        screen_width() / 2. - self.playfield_width() / 2.
    }

    pub fn playfield_width(&self) -> f32 {
        512.
    }

    pub fn movement_direction(&self) -> i32 {
        is_key_down(KeyCode::D) as i32 - is_key_down(KeyCode::A) as i32
    }

    pub fn update(&mut self) {
        let catcher_y = self.catcher_y();

        self.prev_time = self.time;
        self.time = self.handle.position() as f32;

        draw_line(
            self.playfield_to_screen_x(0.) + 2. / 2.,
            0.,
            self.playfield_to_screen_x(0.) + 2. / 2.,
            screen_height(),
            2.,
            RED,
        );

        draw_line(
            self.playfield_to_screen_x(self.playfield_width()) + 2. / 2.,
            0.,
            self.playfield_to_screen_x(self.playfield_width()) + 2. / 2.,
            screen_height(),
            2.,
            RED,
        );

        self.position = self
            .position
            .add(self.movement_direction() as f32 * self.catcher_speed() * get_frame_time() /*FIXED_DELTA*/)
            .clamp(0., self.playfield_width());

        let fruit_travel_distance = self.fruit_y(self.time, 0.) - self.fruit_y(self.prev_time, 0.);

        let catcher_hitbox = Rect::new(
            self.position - 128. / 2.,
            catcher_y - fruit_travel_distance / 2.,
            128.,
            fruit_travel_distance,
        );

        for (idx, fruit) in self.fruits.iter().enumerate() {
            // Last frame hitbox.
            let fruit_hitbox = Rect::new(
                fruit.position - 64. / 2.,
                self.fruit_y(self.prev_time, fruit.time) + 64. / 2. - fruit_travel_distance / 2.,
                64.,
                fruit_travel_distance,
            );
            let hit = can_catch_fruit(catcher_hitbox, fruit_hitbox);
            let miss = fruit_hitbox.y >= screen_height();
            assert!(!(hit && miss), "Can't hit and miss at the same time!");
            if hit {
                self.combo += 1;
                self.deref_delete.push(idx);
            }
            if miss {
                self.combo = 0;
                self.deref_delete.push(idx);
                println!("Miss!");
            }
            if hit || miss {
                self.hyper_multiplier = fruit.hyper.unwrap_or(1.);
            }
        }

        for idx in self.deref_delete.drain(..).rev() {
            self.fruits.remove(idx);
        }
    }

    pub fn draw(&self) {
        let fruit_travel_distance = self.fruit_y(self.time, 0.) - self.fruit_y(self.prev_time, 0.);
        let catcher_hitbox = Rect::new(
            self.position - 128. / 2.,
            self.catcher_y() - fruit_travel_distance / 2.,
            128.,
            fruit_travel_distance,
        );

        for fruit in &self.fruits {
            draw_texture_ex(
                self.fruit,
                self.playfield_to_screen_x(fruit.position) - 64. / 2.,
                self.fruit_y(self.time, fruit.time),
                if fruit.hyper.is_some() { RED } else { WHITE },
                DrawTextureParams {
                    dest_size: Some(vec2(64., 64.)),
                    ..Default::default()
                },
            );
            let fruit_hitbox = Rect::new(
                self.playfield_to_screen_x(fruit.position) - 64. / 2.,
                self.fruit_y(self.time, fruit.time) + 64. / 2.,
                64.,
                fruit_travel_distance / 2.,
            );
            draw_rectangle(
                fruit_hitbox.x,
                fruit_hitbox.y,
                fruit_hitbox.w,
                fruit_hitbox.h,
                BLUE,
            );
        }
        draw_rectangle(
            self.playfield_to_screen_x(catcher_hitbox.x),
            catcher_hitbox.y,
            catcher_hitbox.w,
            catcher_hitbox.h,
            RED,
        );
        draw_texture_ex(
            self.catcher,
            self.playfield_to_screen_x(self.position) - 128. / 2.,
            self.catcher_y(),
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(128., 128.)),
                ..Default::default()
            },
        );

        draw_text(
            &format!("{}x", self.combo),
            5.,
            screen_height() - 5.,
            36.,
            WHITE,
        );
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new().await;
    //let mut counter = 0.;
    loop {
        //let frame_time = get_frame_time();
        //counter += frame_time;
        //while counter >= FIXED_DELTA {
        game.update();
        //    counter -= FIXED_DELTA;
        //}

        clear_background(LIGHTGRAY);
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
