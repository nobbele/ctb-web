use super::{JudgementResult, Ruleset};
use crate::{
    chart::{Chart, Fruit},
    score::{Judgement, Score, ScoreRecorder},
};
use macroquad::prelude::*;

#[derive(
    Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize, Clone, PartialOrd, Ord,
)]
pub enum CatchJudgement {
    Perfect,
    Miss,
}

pub struct CatchHitDetails {
    /// How far from the center it was hit \[-1; 1\].
    pub off: f32,
}

impl Judgement for CatchJudgement {
    fn hit(_inaccuracy: f32) -> Self {
        // There's no accuracy in Catch.
        Self::Perfect
    }

    fn miss() -> Self {
        Self::Miss
    }

    fn weight(&self) -> f32 {
        match self {
            CatchJudgement::Perfect => 1.0,
            CatchJudgement::Miss => 0.0,
        }
    }

    fn all() -> Vec<Self> {
        vec![CatchJudgement::Perfect, CatchJudgement::Miss]
    }
}

pub type CatchScoreRecorder = ScoreRecorder<CatchJudgement>;
pub type CatchScore = Score<CatchJudgement>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CatchSyncFrame {
    pub position: f32,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct CatchInput {
    pub left: bool,
    pub right: bool,
    pub dash: bool,
}

pub fn catcher_speed(dashing: bool, hyper_multiplier: f32) -> f32 {
    let mut mov_speed = 500.;
    if dashing {
        mov_speed *= 2. * hyper_multiplier;
    }
    mov_speed
}

pub struct CatchRuleset {
    // Range: [0-512]
    /// Player's logical position on the playfield.
    pub position: f32,
    // Range: [1-inf]
    /// Speed multiplier for active hyper dash, if any.
    hyper_multiplier: Option<f32>,
}

impl CatchRuleset {
    pub fn new() -> Self {
        CatchRuleset {
            position: 256.,
            hyper_multiplier: None,
        }
    }

    /// Gets the vertical position of the fruit at a specific time.
    fn fruit_height_at(
        current_time: f32,
        object_time: f32,
        fall_time: f32,
        receptor_height: f32,
    ) -> f32 {
        let time_left = object_time - current_time;
        // How far the object travelled from the top of the screen (0) to the receptor (1).
        let progress = 1. - (time_left / fall_time);
        receptor_height * progress
    }
}

impl Ruleset for CatchRuleset {
    type Input = CatchInput;
    type Object = Fruit;
    type Judgement = CatchJudgement;
    type HitDetails = CatchHitDetails;
    type SyncFrame = CatchSyncFrame;

    fn update(&mut self, dt: f32, input: Self::Input, objects: &[Self::Object]) {
        let mut speed = if input.dash { 1000. } else { 500. };
        if let Some(multiplier) = self.hyper_multiplier {
            speed *= multiplier;
        }

        // Apply input to position and clamp the value.
        input.left.then(|| self.position -= speed * dt);
        input.right.then(|| self.position += speed * dt);
        self.position = self.position.clamp(0.0, 512.0);

        // If we hit a hyperfruit, the multiplier needs to set.
        for object in objects {
            self.hyper_multiplier = object.hyper;
        }
    }

    fn generate_sync_frame(&self) -> Self::SyncFrame {
        CatchSyncFrame {
            position: self.position,
        }
    }

    fn handle_sync_frame(&mut self, frame: &Self::SyncFrame) {
        self.position = frame.position;
    }

    fn test_hitobject(
        &self,
        dt: f32,
        time: f32,
        object: Self::Object,
        chart: &Chart,
    ) -> JudgementResult<(Self::Judgement, Self::HitDetails)> {
        let catcher_height = screen_height() - 148.;
        let current_height =
            Self::fruit_height_at(time, object.time, chart.fall_time, catcher_height);
        let prev_height =
            Self::fruit_height_at(time - dt, object.time, chart.fall_time, catcher_height);
        let distance = object.position - self.position;
        let off = distance / chart.catcher_width;

        if off.abs() <= 1. {
            if current_height >= catcher_height && prev_height <= catcher_height {
                return JudgementResult::Hit((CatchJudgement::Perfect, CatchHitDetails { off }));
            }

            if current_height >= screen_height() {
                return JudgementResult::Miss;
            }
        }

        JudgementResult::None
    }
}
