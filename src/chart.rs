use crate::screen::gameplay::catcher_speed;

#[derive(Debug, Copy, Clone)]
pub struct Fruit {
    pub position: f32,
    pub time: f32,
    pub hyper: Option<f32>,
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

pub struct Chart {
    pub fruits: Vec<Fruit>,
    pub fall_time: f32,
    pub fruit_radius: f32,
    pub catcher_width: f32,
}

impl Chart {
    pub fn from_beatmap(beatmap: &osu_parser::Beatmap) -> Self {
        let mut fruits = Vec::with_capacity(beatmap.hit_objects.len());
        for (idx, hitobject) in beatmap.hit_objects.iter().enumerate() {
            let mut fruit = Fruit::from_hitobject(hitobject);

            // If you can't get to the center of the next fruit in time, we need to give the player some extra speed.
            // TODO use same implementation as osu!catch.
            if let Some(next_hitobject) = beatmap.hit_objects.get(idx + 1) {
                let next_fruit = Fruit::from_hitobject(next_hitobject);
                let dist = (next_fruit.position - fruit.position).abs();
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
