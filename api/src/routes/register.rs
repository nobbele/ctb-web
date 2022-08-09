use actix_web::web;
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Debug)]
pub enum RegistrationError {
    InvalidData,
}

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistrationError::InvalidData => {
                writeln!(f, "{}", serde_json::json!({ "error": "invalid-data" }))
            }
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
) -> Result<&'static str, RegistrationError> {
    if req.username.len() > 16 || req.email.len() > 64 {
        return Err(RegistrationError::InvalidData);
    }

    // TODO: Better salt.
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
