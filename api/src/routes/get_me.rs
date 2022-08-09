use actix_web::web;
use serde::Serialize;
use sqlx::PgPool;

use crate::{
    db::{self, Userdata},
    UserIdFromToken,
};

#[derive(Debug)]
pub enum GetMeError {
    NotFound,
}

impl std::fmt::Display for GetMeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GetMeError::NotFound => writeln!(f, "{}", serde_json::json!({ "error": "not-found" })),
        }
    }
}

impl actix_web::error::ResponseError for GetMeError {}

#[derive(Debug, Serialize)]
pub struct GetMeResponse {
    #[serde(flatten)]
    userdata: Userdata,
}

pub async fn get_me(
    pool: web::Data<PgPool>,
    UserIdFromToken(user_id): UserIdFromToken,
) -> Result<web::Json<GetMeResponse>, GetMeError> {
    let userdata = match db::get_user_by_id(pool.get_ref(), user_id).await {
        Ok(d) => d,
        Err(_) => return Err(GetMeError::NotFound),
    };
    Ok(web::Json(GetMeResponse { userdata }))
}
