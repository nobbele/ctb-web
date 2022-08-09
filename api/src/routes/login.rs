use actix_web::web;
use serde::{Deserialize, Serialize};
use sqlx::{types::Uuid, PgPool};

#[derive(Debug)]
pub enum LoginError {
    InvalidCredentials,
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::InvalidCredentials => {
                writeln!(
                    f,
                    "{}",
                    serde_json::json!({ "error": "invalid-credentials" })
                )
            }
        }
    }
}

impl actix_web::error::ResponseError for LoginError {}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    token: String,
}

pub async fn login(
    pool: web::Data<PgPool>,
    web::Json(req): web::Json<LoginRequest>,
) -> Result<web::Json<LoginResponse>, LoginError> {
    let config = argon2::Config::default();
    let hash = argon2::hash_encoded(
        req.password.as_bytes(),
        std::env::var("PW_SECRET").unwrap().as_bytes(),
        &config,
    )
    .unwrap();

    let user_id: i32 =
        match sqlx::query_as("SELECT user_id FROM users WHERE password = $1 AND username = $2;")
            .bind(hash)
            .bind(&req.username)
            .fetch_optional(pool.get_ref())
            .await
            .unwrap()
        {
            Some((user_id,)) => user_id,
            None => return Err(LoginError::InvalidCredentials),
        };

    let (token,): (Uuid,) = sqlx::query_as("INSERT INTO sessions VALUES ($1) RETURNING token;")
        .bind(user_id)
        .fetch_one(pool.get_ref())
        .await
        .unwrap();

    Ok(web::Json(LoginResponse {
        token: token
            .as_simple()
            .encode_upper(&mut Uuid::encode_buffer())
            .to_owned(),
    }))
}
