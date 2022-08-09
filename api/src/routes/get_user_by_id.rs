use actix_web::web;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db::{self, Userdata};

#[derive(Debug)]
pub enum GetUserByIdError {
    NotFound,
}

impl std::fmt::Display for GetUserByIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GetUserByIdError::NotFound => {
                writeln!(f, "{}", serde_json::json!({ "error": "not-found" }))
            }
        }
    }
}

impl actix_web::error::ResponseError for GetUserByIdError {}

#[derive(Debug, Deserialize)]
pub struct GetUserByIdPath {
    user_id: u32,
}

#[derive(Debug, Serialize)]
pub struct GetUserByIdResponse {
    #[serde(flatten)]
    userdata: Userdata,
}

pub async fn get_user_by_id(
    pool: web::Data<PgPool>,
    path: web::Path<GetUserByIdPath>,
) -> Result<web::Json<GetUserByIdResponse>, GetUserByIdError> {
    let user_id = path.into_inner().user_id;
    let userdata = match db::get_user_by_id(pool.get_ref(), user_id).await {
        Ok(d) => d,
        Err(_) => return Err(GetUserByIdError::NotFound),
    };
    Ok(web::Json(GetUserByIdResponse { userdata }))
}
