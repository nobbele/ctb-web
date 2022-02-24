use crate::{score::Score, screen::GameData};
use std::sync::Arc;

pub async fn submit_score(data: Arc<GameData>, score: &Score) {
    data.glue
        .lock()
        .execute_async(&format!(
            include_str!("queries/insert_leaderboard.sql"),
            score.diff_id, score.hit_count, score.miss_count, score.score, score.top_combo
        ))
        .await
        .unwrap();
}
