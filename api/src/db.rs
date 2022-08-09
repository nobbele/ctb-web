use serde::Serialize;
use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn init_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new().connect(database_url).await
}

#[derive(Debug, Serialize)]
pub struct Userdata {
    id: u32,
    username: String,
    email: Option<String>,
}

pub async fn get_user_by_id(pool: &PgPool, id: u32) -> Result<Userdata, ()> {
    let (username, email): (String, String) =
        match sqlx::query_as("SELECT username, email FROM users WHERE user_id = $1;")
            .bind(i32::try_from(id).unwrap())
            .fetch_optional(pool)
            .await
            .unwrap()
        {
            Some(data) => data,
            None => return Err(()),
        };
    Ok(Userdata {
        id,
        username,
        email: Some(email),
    })
}
