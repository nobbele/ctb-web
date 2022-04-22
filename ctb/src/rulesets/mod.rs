use crate::{chart::Chart, score::Judgement};

pub mod catch;

pub trait Ruleset {
    /// Input type for this ruleset.
    type Input;

    /// (Hit-)object type for this ruleset.
    type Object;

    /// Judgement type for this ruleset. An indication of how well an object was hit.
    type Judgement: Judgement;

    /// Sync frames are used to synchronize replays periodically.
    type SyncFrame;

    /// Run a frame of the ruleset.
    ///
    /// `dt` in this case refers to frame delta-time.
    ///
    /// `objects` contains fruits that were hit since the last call.
    fn update(&mut self, dt: f32, input: Self::Input, objects: &[Self::Object]);

    /// Generate a sync frame at this moment for use in replays.
    fn generate_sync_frame(&self) -> Self::SyncFrame;

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
    ) -> Option<Self::Judgement>;
}
