use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn init_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .connect_timeout(std::time::Duration::from_secs(1))
        .connect(database_url)
        .await
}
