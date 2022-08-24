use macroquad::prelude::Color;

/// Represents hitsound additions.
#[derive(Debug, Copy, Clone)]
pub struct Additions {
    pub whistle: bool,
    pub finish: bool,
    pub clap: bool,
}

/// Represents a catch fruit.
#[derive(Debug, Copy, Clone)]
pub struct Fruit {
    pub position: f32,
    pub time: f32,
    pub hyper: Option<f32>,
    pub small: bool,
    pub additions: Additions,
    pub color: Color,
    pub plate_reset: bool,
    pub fall_multiplier: f32,
}

impl Fruit {
    /// Calculate the angle from `self` to `other` where they fall across the screen in `fall_time` seconds.
    pub fn angle_to(&self, other: &Fruit, fall_time: f32) -> f32 {
        let time_to_hit = other.time.max(self.time) - other.time.min(self.time);
        const H: f32 = 768.;
        let jump_height = time_to_hit * H / fall_time;
        let jump_width = (other.position - self.position).abs();

        (jump_height / jump_width).atan()
    }
}

#[derive(Debug, Clone)]
pub enum HitSoundKind {
    Normal,
    Soft,
    Drum,
    Custom(String),
}

#[derive(Debug)]
pub enum EventData {
    Timing { bpm: f32 },
    Hitsound { kind: HitSoundKind, volume: f32 },
}

#[derive(Debug)]
pub struct Event {
    pub time: f32,
    pub data: EventData,
}

pub struct Chart {
    pub fruits: Vec<Fruit>,
    pub events: Vec<Event>,
    pub fall_time: f32,
    pub fruit_radius: f32,
    pub catcher_width: f32,
}
