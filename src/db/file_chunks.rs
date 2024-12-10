use sqlx::{Pool, Postgres};
use crate::models::file_chunk::FileChunk;
use uuid::Uuid;

pub async fn create_file_chunk(
    pool: &Pool<Postgres>,
    file_id: Uuid,
    chunk_index: i32,
    checksum: &str,
    size: i64
) -> Result<FileChunk, sqlx::Error> {
    let record = sqlx::query_as!(FileChunk,
        r#"
        INSERT INTO file_chunks (file_id, chunk_index, checksum, size)
        VALUES ($1, $2, $3, $4)
        RETURNING chunk_id, file_id, chunk_index, checksum, size, created_at
        "#,
        file_id, chunk_index, checksum, size
    )
        .fetch_one(pool)
        .await?;
    Ok(record)
}

pub async fn get_chunks_by_file_id(pool: &Pool<Postgres>, file_id: Uuid) -> Result<Vec<FileChunk>, sqlx::Error> {
    let records = sqlx::query_as!(FileChunk,
        r#"
        SELECT chunk_id, file_id, chunk_index, checksum, size, created_at
        FROM file_chunks
        WHERE file_id = $1
        ORDER BY chunk_index
        "#,
        file_id
    )
        .fetch_all(pool)
        .await?;
    Ok(records)
}
