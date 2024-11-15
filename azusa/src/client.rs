use crate::app::{App, Target};
use client::{
    azusa::{ClientPacket, ServerPacket},
    chat::{ChatMessage, ChatMessagePacket},
    rulesets::{
        catch::{CatchJudgement, CatchScore},
        JudgementResult,
    },
};
use sqlx::Row;
use std::{collections::BTreeMap, time::Instant};

pub struct Client {
    last_ping: Instant,
    user_id: u32,
    username: String,
    app: &'static App,
}

impl Client {
    pub fn new(username: String, user_id: u32, app: &'static App) -> Self {
        app.send(
            Target::User(username.clone()),
            ServerPacket::Chat(ChatMessagePacket(ChatMessage {
                username: "Azusa".to_owned(),
                content: "Welcome to ctb-web!".to_owned(),
            })),
        );

        Client {
            username,
            user_id,
            last_ping: Instant::now(),
            app,
        }
    }
}

impl Client {
    pub async fn handle(&mut self, packet: ClientPacket) {
        match packet {
            ClientPacket::Echo(s) => {
                self.app
                    .send(Target::User(self.username.clone()), ServerPacket::Echo(s));
            }
            ClientPacket::Ping => {
                self.app
                    .send(Target::User(self.username.clone()), ServerPacket::Pong);
                self.last_ping = Instant::now();
                self.app.send(
                    Target::User(self.username.clone()),
                    ServerPacket::Chat(ChatMessagePacket(ChatMessage {
                        username: "Azusa".to_owned(),
                        content: "Ping-Pong".to_owned(),
                    })),
                );
            }
            ClientPacket::Chat(content) => {
                self.app.send(
                    Target::Everyone,
                    ServerPacket::Chat(ChatMessagePacket(ChatMessage {
                        username: self.username.clone(),
                        content,
                    })),
                );
            }
            ClientPacket::Login(_) => panic!("Can't login after already being logged in!"),
            ClientPacket::Submit(score) => {
                println!("Submitting score for {}", self.username);
                sqlx::query("INSERT INTO scores(user_id, diff_id, hit_count, miss_count, score, top_combo) VALUES ($1, $2, $3, $4, $5, $6)")
                .bind(i32::try_from(self.user_id).unwrap())
                .bind(i32::try_from(score.diff_id).unwrap())
                .bind(i32::try_from(score.judgements[&JudgementResult::Hit(CatchJudgement::Perfect)]).unwrap())
                .bind(i32::try_from(score.judgements[&JudgementResult::Miss]).unwrap())
                .bind(i32::try_from(score.score).unwrap())
                .bind(i32::try_from(score.top_combo).unwrap()).execute(&self.app.pool).await.unwrap();
            }
            ClientPacket::RequestLeaderboard(diff_id) => {
                let scores = sqlx::query(
                    "
                    SELECT username, hit_count, miss_count, score, top_combo
                        FROM scores
                        INNER JOIN users ON (users.user_id = scores.user_id)
                        WHERE diff_id = $1
                        ORDER BY score DESC
                        ",
                )
                .bind(i32::try_from(diff_id).unwrap())
                .map(|row: sqlx::postgres::PgRow| {
                    let username: String = row.try_get(0).unwrap();
                    let hit_count: i32 = row.try_get(1).unwrap();
                    let miss_count: i32 = row.try_get(2).unwrap();
                    let score: i32 = row.try_get(3).unwrap();
                    let top_combo: i32 = row.try_get(4).unwrap();
                    CatchScore {
                        username: Some(username),
                        diff_id,
                        score: score.try_into().unwrap(),
                        top_combo: top_combo.try_into().unwrap(),
                        passed: true,
                        judgements: {
                            let mut judgements = BTreeMap::new();
                            judgements.insert(
                                JudgementResult::Hit(CatchJudgement::Perfect),
                                hit_count.try_into().unwrap(),
                            );
                            judgements
                                .insert(JudgementResult::Miss, miss_count.try_into().unwrap());
                            judgements
                        },
                    }
                })
                .fetch_all(&self.app.pool)
                .await
                .unwrap();
                self.app.send(
                    Target::User(self.username.clone()),
                    ServerPacket::Leaderboard { diff_id, scores },
                );
            }
            ClientPacket::Goodbye => todo!(),
        }
    }
}
