use crate::{chart::Chart, score::Judgement};

pub mod catch;

#[derive(
    Debug, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize, Clone, PartialOrd, Ord,
)]
pub enum JudgementResult<H> {
    Hit(H),
    Miss,
}

impl<H> JudgementResult<H> {
    pub fn map_hit<T>(self, f: impl FnOnce(H) -> T) -> JudgementResult<T> {
        match self {
            JudgementResult::Hit(h) => JudgementResult::Hit(f(h)),
            JudgementResult::Miss => JudgementResult::Miss,
        }
    }
}

pub trait Ruleset {
    /// Input type for this ruleset.
    type Input: serde::Serialize + for<'a> serde::Deserialize<'a>;

    /// (Hit-)object type for this ruleset.
    type Object;

    /// Judgement type for this ruleset. An indication of how well an object was hit.
    type Judgement: Judgement;

    /// Judgement type for this ruleset. An indication of how well an object was hit.
    type HitDetails;

    /// Sync frames are used to synchronize replays periodically.
    type SyncFrame: serde::Serialize + for<'a> serde::Deserialize<'a>;

    /// Run a frame of the ruleset.
    ///
    /// `dt` in this case refers to frame delta-time.
    ///
    /// `objects` contains fruits that were hit since the last call.
    fn update(&mut self, dt: f32, input: Self::Input, objects: &[Self::Object]);

    /// Generate a sync frame at this moment for use in replays.
    fn generate_sync_frame(&self) -> Self::SyncFrame;

    /// Sync frame handler, used to synchronize the current ruelset state with the replay.
    fn handle_sync_frame(&mut self, frame: &Self::SyncFrame);

    /// Tests if an object was hit.
    ///
    /// `dt` in this case refers to the audio delta-time.
    ///
    /// Returns the judgement to be recorded for the object, if any.
    fn test_hitobject(
        &self,
        dt: f32,
        time: f32,
        object: Self::Object,
        chart: &Chart,
    ) -> Option<JudgementResult<(Self::Judgement, Self::HitDetails)>>;
}
