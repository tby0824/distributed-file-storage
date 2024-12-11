use sqlx::Pool;
use sqlx::Postgres;
use anyhow::Result;

pub async fn init_pool(db_url: &str) -> Result<Pool<Postgres>> {
    let pool = Pool::<Postgres>::connect(db_url).await?;
    Ok(pool)
}

pub mod queries;
