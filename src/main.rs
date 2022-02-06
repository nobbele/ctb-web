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

#[derive(Debug, Copy, Clone)]
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

struct Chart {
    fruits: Vec<Fruit>,
    fall_time: f32,
    fruit_radius: f32,
    catcher_width: f32,
}

impl Chart {
    pub fn from_beatmap(beatmap: &osu_parser::Beatmap) -> Self {
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

        Chart {
            fruits,
            fall_time: osu_utils::ar_to_ms(beatmap.info.difficulty.ar) / 1000.,
            fruit_radius: osu_utils::cs_to_px(beatmap.info.difficulty.cs),
            catcher_width: {
                let scale = 1. - 0.7 * (beatmap.info.difficulty.cs - 5.) / 5.;
                106.75 * scale * 0.8
            },
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

    chart: Chart,
    queued_fruits: Vec<usize>,
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
        let chart = Chart::from_beatmap(&beatmap);

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
            deref_delete: Vec::with_capacity(chart.fruits.len()),
            queued_fruits: (0..chart.fruits.len()).collect(),
            chart,
        }
    }

    pub fn catcher_y(&self) -> f32 {
        screen_height() - 148.
    }

    pub fn fruit_y(&self, time: f32, target: f32) -> f32 {
        let time_left = target - time;
        let progress = 1. - (time_left / self.chart.fall_time);
        self.catcher_y() * progress
    }

    pub fn catcher_speed(&self) -> f32 {
        catcher_speed(is_key_down(KeyCode::RightShift), self.hyper_multiplier)
    }

    pub fn playfield_to_screen_x(&self, x: f32) -> f32 {
        let visual_width = self.playfield_width() * self.scale();
        let playfield_x = screen_width() / 2. - visual_width / 2.;
        playfield_x + x * self.scale()
    }

    pub fn scale(&self) -> f32 {
        let scale = screen_width() / 512.;
        scale * 2. / 3.
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

        self.position = self
            .position
            .add(self.movement_direction() as f32 * self.catcher_speed() * get_frame_time() /*FIXED_DELTA*/)
            .clamp(0., self.playfield_width());

        let fruit_travel_distance = self.fruit_y(self.time, 0.) - self.fruit_y(self.prev_time, 0.);

        let catcher_hitbox = Rect::new(
            self.position - self.chart.catcher_width / 2.,
            catcher_y - fruit_travel_distance / 2.,
            self.chart.catcher_width,
            fruit_travel_distance,
        );

        for (idx, fruit) in self.queued_fruits.iter().enumerate() {
            let fruit = self.chart.fruits[*fruit];
            // Last frame hitbox.
            let fruit_hitbox = Rect::new(
                fruit.position - self.chart.fruit_radius,
                self.fruit_y(self.prev_time, fruit.time) - fruit_travel_distance / 2.,
                self.chart.fruit_radius * 2.,
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
            self.queued_fruits.remove(idx);
        }
    }

    pub fn draw(&self) {
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

        let fruit_travel_distance = self.fruit_y(self.time, 0.) - self.fruit_y(self.prev_time, 0.);
        let catcher_hitbox = Rect::new(
            self.position - self.chart.catcher_width / 2.,
            self.catcher_y() - fruit_travel_distance / 2.,
            self.chart.catcher_width,
            fruit_travel_distance,
        );

        for fruit in &self.queued_fruits {
            let fruit = self.chart.fruits[*fruit];
            draw_texture_ex(
                self.fruit,
                self.playfield_to_screen_x(fruit.position) - self.chart.fruit_radius * self.scale(),
                self.fruit_y(self.time, fruit.time) - self.chart.fruit_radius * self.scale(),
                if fruit.hyper.is_some() { RED } else { WHITE },
                DrawTextureParams {
                    dest_size: Some(vec2(
                        self.chart.fruit_radius * 2. * self.scale(),
                        self.chart.fruit_radius * 2. * self.scale(),
                    )),
                    ..Default::default()
                },
            );
            let fruit_hitbox = Rect::new(
                fruit.position - self.chart.fruit_radius,
                self.fruit_y(self.time, fruit.time) - fruit_travel_distance / 2.,
                self.chart.fruit_radius * 2.,
                fruit_travel_distance,
            );
            let prev_fruit_hitbox = Rect::new(
                fruit.position - self.chart.fruit_radius,
                self.fruit_y(self.prev_time, fruit.time) - fruit_travel_distance / 2.,
                self.chart.fruit_radius * 2.,
                fruit_travel_distance,
            );
            draw_rectangle(
                self.playfield_to_screen_x(fruit_hitbox.x),
                fruit_hitbox.y,
                fruit_hitbox.w * self.scale(),
                fruit_hitbox.h,
                BLUE,
            );
            draw_rectangle(
                self.playfield_to_screen_x(prev_fruit_hitbox.x),
                prev_fruit_hitbox.y,
                prev_fruit_hitbox.w * self.scale(),
                prev_fruit_hitbox.h,
                GREEN,
            );
        }
        draw_rectangle(
            self.playfield_to_screen_x(catcher_hitbox.x),
            catcher_hitbox.y,
            catcher_hitbox.w * self.scale(),
            catcher_hitbox.h,
            RED,
        );
        draw_texture_ex(
            self.catcher,
            self.playfield_to_screen_x(self.position)
                - self.chart.catcher_width * self.scale() / 2.,
            self.catcher_y(),
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(
                    self.chart.catcher_width * self.scale(),
                    self.chart.catcher_width * self.scale(),
                )),
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
