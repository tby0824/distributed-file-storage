use sqlx::{Pool, Postgres};

pub mod users;
pub mod files;
pub mod file_chunks;
pub mod nodes;
pub mod chunk_nodes;

pub async fn init_db(database_url: &str) -> Result<Pool<Postgres>, sqlx::Error> {
    let pool = Pool::<Postgres>::connect(database_url).await?;
    // 可在此处执行数据库迁移
    Ok(pool)
}
