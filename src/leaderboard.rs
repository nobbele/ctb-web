use crate::score::Score;
#[cfg(not(target_family = "wasm"))]
use {
    gluesql::prelude::{Glue, Payload, SledStorage, Value},
    std::{collections::HashMap, ops::Deref},
};

#[derive(Debug, Clone)]
pub struct LeaderboardEntry {
    pub score: u32,
    pub accuracy: f32,
}

pub struct Leaderboard {
    #[cfg(not(target_family = "wasm"))]
    glue: Glue<gluesql::sled_storage::sled::IVec, SledStorage>,
}

#[cfg(not(target_family = "wasm"))]
impl Leaderboard {
    pub async fn new() -> Self {
        let storage = SledStorage::new("data/.storage").unwrap();
        let mut glue = Glue::new(storage);

        /*glue.execute_async("DROP TABLE IF EXISTS 'scores'; DROP TABLE IF EXISTS 'maps'; DROP TABLE IF EXISTS 'diffs'; ")
        .await
        .unwrap();*/
        glue.execute_async(include_str!("queries/initialize.sql"))
            .await
            .unwrap();
        Leaderboard { glue }
    }

    pub async fn submit_score(&mut self, score: &Score) {
        self.glue
            .execute_async(&format!(
                include_str!("queries/insert_leaderboard.sql"),
                score.diff_id, score.hit_count, score.miss_count, score.score, score.top_combo
            ))
            .await
            .unwrap();
    }

    pub async fn get_local(&mut self, diff_id: u32) -> Vec<LeaderboardEntry> {
        let leaderboard = self
            .glue
            .execute_async(&format!(
                include_str!("queries/local_leaderboard.sql"),
                diff_id
            ))
            .await
            .unwrap();
        let mut entries = Vec::new();
        match leaderboard {
            Payload::Select { labels, rows } => {
                for row in rows {
                    let map = labels
                        .iter()
                        .map(Deref::deref)
                        .zip(row.iter().map(|col| match col {
                            Value::I64(v) => *v as u32,
                            _ => unreachable!(),
                        }))
                        .collect::<HashMap<_, _>>();
                    let entry = LeaderboardEntry {
                        score: map["score"],
                        accuracy: map["hit_count"] as f32
                            / (map["hit_count"] + map["miss_count"]) as f32,
                    };
                    entries.push(entry);
                }
            }
            _ => unreachable!(),
        }

        entries
    }
}

#[cfg(target_family = "wasm")]
impl Leaderboard {
    pub async fn new() -> Self {
        Leaderboard {}
    }

    pub async fn submit_score(&mut self, _score: &Score) {}

    pub async fn get_local(&mut self, _diff_id: u32) -> Vec<LeaderboardEntry> {
        Vec::new()
    }
}
