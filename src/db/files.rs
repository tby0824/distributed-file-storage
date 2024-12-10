use sqlx::{Pool, Postgres};
use crate::models::file::File;
use uuid::Uuid;

pub async fn create_file(pool: &Pool<Postgres>, owner_id: Option<Uuid>, file_name: &str, file_size: i64) -> Result<File, sqlx::Error> {
    let record = sqlx::query_as!(File,
        r#"
        INSERT INTO files (owner_id, file_name, file_size)
        VALUES ($1, $2, $3)
        RETURNING file_id, owner_id, file_name, file_size, created_at, updated_at
        "#,
        owner_id, file_name, file_size
    )
        .fetch_one(pool)
        .await?;
    Ok(record)
}

pub async fn get_file_by_id(pool: &Pool<Postgres>, file_id: Uuid) -> Result<File, sqlx::Error> {
    let record = sqlx::query_as!(File,
        r#"
        SELECT file_id, owner_id, file_name, file_size, created_at, updated_at
        FROM files
        WHERE file_id = $1
        "#,
        file_id
    )
        .fetch_one(pool)
        .await?;
    Ok(record)
}

pub async fn list_user_files(pool: &Pool<Postgres>, user_id: Uuid) -> Result<Vec<File>, sqlx::Error> {
    let records = sqlx::query_as!(File,
        r#"
        SELECT file_id, owner_id, file_name, file_size, created_at, updated_at
        FROM files
        WHERE owner_id = $1
        ORDER BY created_at DESC
        "#,
        user_id
    )
        .fetch_all(pool)
        .await?;
    Ok(records)
}

pub async fn list_all_files(pool: &Pool<Postgres>) -> Result<Vec<File>, sqlx::Error> {
    let records = sqlx::query_as!(File,
        r#"
        SELECT file_id, owner_id, file_name, file_size, created_at, updated_at
        FROM files
        ORDER BY created_at DESC
        "#,
    )
        .fetch_all(pool)
        .await?;
    Ok(records)
}

pub async fn delete_file(pool: &Pool<Postgres>, file_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM files WHERE file_id = $1",
        file_id
    )
        .execute(pool)
        .await?;
    Ok(())
}
