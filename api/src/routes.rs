use actix_web::{web, Responder};
use serde::{Deserialize, Serialize};
use sqlx::{types::Uuid, PgPool};

#[derive(Debug)]
pub enum LoginError {
    InvalidCredentials,
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::InvalidCredentials => writeln!(f, "Invalid Credentials"),
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
) -> Result<impl Responder, LoginError> {
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
    let user_id = u32::try_from(user_id).unwrap();

    let (token,): (Uuid,) = sqlx::query_as("INSERT INTO sessions VALUES ($1) RETURNING token;")
        .bind(user_id)
        .fetch_one(pool.get_ref())
        .await
        .unwrap();

    Ok(web::Json(LoginResponse {
        token: token
            .to_simple()
            .encode_upper(&mut Uuid::encode_buffer())
            .to_owned(),
    }))
}

#[derive(Debug)]
pub enum RegistrationError {
    InvalidData,
}

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistrationError::InvalidData => writeln!(f, "Invalid Data"),
        }
    }
}

impl actix_web::error::ResponseError for RegistrationError {}

#[derive(Debug, Deserialize)]
pub struct RegistrationRequest {
    username: String,
    email: String,
    password: String,
}

pub async fn register(
    pool: web::Data<PgPool>,
    web::Json(req): web::Json<RegistrationRequest>,
) -> Result<impl Responder, RegistrationError> {
    if req.username.len() > 16 || req.email.len() > 64 {
        return Err(RegistrationError::InvalidData);
    }

    let config = argon2::Config::default();
    let hash = argon2::hash_encoded(
        req.password.as_bytes(),
        std::env::var("PW_SECRET").unwrap().as_bytes(),
        &config,
    )
    .unwrap();

    sqlx::query("INSERT INTO users(username, email, password) VALUES ($1, $2, $3);")
        .bind(req.username)
        .bind(req.email)
        .bind(hash)
        .execute(pool.get_ref())
        .await
        .unwrap();

    Ok("Success")
}
