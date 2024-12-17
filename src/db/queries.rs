use sqlx::{Pool, Postgres, Transaction};
use anyhow::Result;
use uuid::Uuid;
use crate::models::{User, FileMeta, FileChunk, Node};
use chrono::{DateTime, Utc};

pub async fn get_user_by_username(pool: &Pool<Postgres>, username: &str) -> Result<Option<User>> {
    let res = sqlx::query_as!(
        User,
        r#"SELECT user_id, username, password, email, role, created_at, updated_at
           FROM users WHERE username = $1"#,
        username
    )
        .fetch_optional(pool)
        .await?;
    Ok(res)
}

pub async fn get_user_by_email(pool: &Pool<Postgres>, email: &str) -> Result<Option<User>> {
    let res = sqlx::query_as!(
        User,
        r#"SELECT user_id, username, password, email, role, created_at, updated_at
           FROM users WHERE email = $1"#,
        email
    )
        .fetch_optional(pool)
        .await?;
    Ok(res)
}

pub async fn insert_user(pool: &Pool<Postgres>, username: &str, password: &str, email: Option<&str>) -> Result<Uuid> {
    let rec = sqlx::query!(
        "INSERT INTO users (username, password, email) VALUES ($1, $2, $3) RETURNING user_id",
        username, password, email
    )
        .fetch_one(pool)
        .await?;
    Ok(rec.user_id)
}

pub async fn get_file_by_name(pool: &Pool<Postgres>, owner_id: Uuid, file_name: &str) -> Result<Option<FileMeta>> {
    let res = sqlx::query_as!(
        FileMeta,
        r#"SELECT file_id, owner_id, file_name, file_size, created_at, updated_at
           FROM files
           WHERE owner_id = $1 AND file_name = $2"#,
        owner_id, file_name
    )
        .fetch_optional(pool)
        .await?;
    Ok(res)
}

pub async fn get_file_by_id(pool: &Pool<Postgres>, file_id: Uuid) -> Result<Option<FileMeta>> {
    let res = sqlx::query_as!(
        FileMeta,
        r#"SELECT file_id, owner_id, file_name, file_size, created_at, updated_at
           FROM files
           WHERE file_id = $1"#,
        file_id
    )
        .fetch_optional(pool)
        .await?;
    Ok(res)
}

pub async fn insert_file_meta_tx(tx: &mut Transaction<'_, Postgres>, owner_id: Uuid, file_name: &str, file_size: i64) -> Result<Uuid> {
    let rec = sqlx::query!(
        "INSERT INTO files (owner_id, file_name, file_size) VALUES ($1, $2, $3) RETURNING file_id",
        owner_id, file_name, file_size
    )
        .fetch_one(&mut **tx)
        .await?;
    Ok(rec.file_id)
}

pub async fn delete_file_by_id_tx(tx: &mut Transaction<'_, Postgres>, file_id: Uuid) -> Result<u64> {
    sqlx::query!(
        "DELETE FROM chunk_nodes WHERE chunk_id IN (SELECT chunk_id FROM file_chunks WHERE file_id = $1)",
        file_id
    ).execute(&mut **tx).await?;
    sqlx::query!(
        "DELETE FROM file_chunks WHERE file_id = $1",
        file_id
    ).execute(&mut **tx).await?;

    let rows = sqlx::query!(
        "DELETE FROM files WHERE file_id = $1",
        file_id
    )
        .execute(&mut **tx)
        .await?
        .rows_affected();
    Ok(rows)
}

pub async fn list_user_files(pool: &Pool<Postgres>, owner_id: Uuid) -> Result<Vec<FileMeta>> {
    let files = sqlx::query_as!(
        FileMeta,
        r#"SELECT file_id, owner_id, file_name, file_size, created_at, updated_at
           FROM files WHERE owner_id = $1
           ORDER BY created_at ASC"#,
        owner_id
    )
        .fetch_all(pool)
        .await?;
    Ok(files)
}

pub async fn rename_file(pool: &Pool<Postgres>, owner_id: Uuid, old_name: &str, new_name: &str) -> Result<u64> {
    let rows = sqlx::query!(
        "UPDATE files SET file_name = $1, updated_at = now() WHERE owner_id = $2 AND file_name = $3",
        new_name,
        owner_id,
        old_name
    )
        .execute(pool)
        .await?
        .rows_affected();
    Ok(rows)
}

pub async fn can_read_file(_pool: &Pool<Postgres>, _user_id: Uuid, _file_id: Uuid) -> Result<bool> {
    Ok(true)
}

pub async fn can_write_file(_pool: &Pool<Postgres>, _user_id: Uuid, _file_id: Uuid) -> Result<bool> {
    Ok(true)
}

pub async fn list_nodes(pool: &Pool<Postgres>) -> Result<Vec<Node>> {
    let nodes = sqlx::query_as!(
        Node,
        r#"SELECT node_id, node_address, status, last_heartbeat, created_at, updated_at FROM nodes
           WHERE status = 'active' ORDER BY created_at ASC"#
    )
        .fetch_all(pool)
        .await?;
    Ok(nodes)
}

pub async fn insert_node(pool: &Pool<Postgres>, node_address: &str) -> Result<Uuid> {
    let rec = sqlx::query!(
        "INSERT INTO nodes (node_address) VALUES ($1) RETURNING node_id",
        node_address
    ).fetch_one(pool)
        .await?;
    Ok(rec.node_id)
}

pub async fn insert_file_chunk_tx(tx: &mut Transaction<'_, Postgres>, file_id: Uuid, chunk_index: i32, checksum: &str, size: i64) -> Result<Uuid> {
    let rec = sqlx::query!(
        "INSERT INTO file_chunks (file_id, chunk_index, checksum, size) VALUES ($1, $2, $3, $4) RETURNING chunk_id",
        file_id, chunk_index, checksum, size
    )
        .fetch_one(&mut **tx)
        .await?;
    Ok(rec.chunk_id)
}
pub async fn delete_file_chunks_tx(tx: &mut Transaction<'_, Postgres>, file_id: Uuid) -> Result<u64> {
    let result = sqlx::query!(
        "DELETE FROM file_chunks WHERE file_id = $1",
        file_id
    )
    .execute(&mut **tx)
    .await?;
    
    Ok(result.rows_affected())
}
pub async fn delete_chunk_nodes_by_file_tx(tx: &mut Transaction<'_, Postgres>, file_id: Uuid) -> Result<u64> {
    let result = sqlx::query!(
        r#"DELETE FROM chunk_nodes 
        WHERE chunk_id IN (
            SELECT chunk_id 
            FROM file_chunks 
            WHERE file_id = $1
        )"#,
        file_id
    )
    .execute(&mut **tx)
    .await?;
    
    Ok(result.rows_affected())
}


pub async fn get_file_chunks(pool: &Pool<Postgres>, file_id: Uuid) -> Result<Vec<FileChunk>> {
    let chunks = sqlx::query_as!(
        FileChunk,
        r#"SELECT chunk_id, file_id, chunk_index, checksum, size, created_at
           FROM file_chunks
           WHERE file_id = $1
           ORDER BY chunk_index ASC"#,
        file_id
    )
        .fetch_all(pool)
        .await?;
    Ok(chunks)
}

pub async fn insert_chunk_node_tx(tx: &mut Transaction<'_, Postgres>, chunk_id: Uuid, node_id: Uuid, replica_index: i32) -> Result<()> {
    sqlx::query!(
        r#"INSERT INTO chunk_nodes (chunk_id, node_id, replica_index, status) 
           VALUES ($1, $2, $3, 'active')"#,
        chunk_id, 
        node_id, 
        replica_index
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn update_node_heartbeat(pool: &Pool<Postgres>, node_id: Uuid, time: DateTime<Utc>) -> Result<()> {
    sqlx::query!(
        "UPDATE nodes SET last_heartbeat = $1, updated_at = now() WHERE node_id = $2",
        time.naive_utc(), // Convert to NaiveDateTime
        node_id
    ).execute(pool).await?;
    Ok(())
}

pub async fn deactivate_stale_nodes(pool: &Pool<Postgres>, cutoff: DateTime<Utc>) -> Result<()> {
    // Start a transaction to ensure both updates are atomic
    let mut tx = pool.begin().await?;

    // Get the node IDs that will be deactivated and update their status
    let deactivated_nodes = sqlx::query!(
        r#"
        WITH deactivated AS (
            UPDATE nodes 
            SET status = 'inactive', 
                updated_at = now() 
            WHERE (last_heartbeat < $1 OR last_heartbeat IS NULL) 
            AND status = 'active'
            RETURNING node_id
        )
        SELECT node_id FROM deactivated
        "#,
        cutoff.naive_utc()
    )
    .fetch_all(&mut *tx)
    .await?;

    // If any nodes were deactivated, update their associated chunks
    if !deactivated_nodes.is_empty() {
        let node_ids: Vec<_> = deactivated_nodes.iter().map(|row| &row.node_id).collect();
        
        sqlx::query!(
            r#"
            UPDATE chunk_nodes 
            SET status = 'inactive',
                updated_at = now()
            WHERE node_id = ANY($1)
            AND status = 'active'
            "#,
            &node_ids as &[_]
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    
    Ok(())
}