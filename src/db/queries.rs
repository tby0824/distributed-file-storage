use sqlx::{Pool, Postgres, Row};
use anyhow::Result;
use uuid::Uuid;
use crate::models::{User, FileMeta, FileChunk, Node, ChunkNode};
use chrono::Utc;

pub async fn create_user(pool: &Pool<Postgres>, username: &str, password: &str, email: Option<&str>) -> Result<User> {
    let rec = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, password, email) VALUES ($1, $2, $3)
         RETURNING user_id, username, password, email, role, created_at, updated_at"
    )
        .bind(username)
        .bind(password)
        .bind(email)
        .fetch_one(pool)
        .await?;
    Ok(rec)
}



pub async fn get_user_by_username(pool: &Pool<Postgres>, username: &str) -> Result<Option<User>> {
    let res = sqlx::query_as::<_, User>(
        "SELECT user_id, username, password, email, role, created_at, updated_at
         FROM users WHERE username = $1"
    )
        .bind(username)
        .fetch_optional(pool)
        .await?;
    Ok(res)
}

pub async fn create_file_metadata(pool: &Pool<Postgres>, owner_id: Uuid, file_name: &str, file_size: i64) -> Result<FileMeta> {
    let res = sqlx::query_as::<_, FileMeta>(
        "INSERT INTO files (owner_id, file_name, file_size) VALUES ($1, $2, $3)
         RETURNING file_id, owner_id, file_name, file_size, created_at, updated_at"
    )
        .bind(owner_id)
        .bind(file_name)
        .bind(file_size)
        .fetch_one(pool)
        .await?;
    Ok(res)
}

pub async fn create_file_chunk(pool: &Pool<Postgres>, file_id: Uuid, chunk_index: i32, checksum: &str, size: i64) -> Result<FileChunk> {
    let res = sqlx::query_as::<_, FileChunk>(
        "INSERT INTO file_chunks (file_id, chunk_index, checksum, size) VALUES ($1, $2, $3, $4)
         RETURNING chunk_id, file_id, chunk_index, checksum, size, created_at"
    )
        .bind(file_id)
        .bind(chunk_index)
        .bind(checksum)
        .bind(size)
        .fetch_one(pool)
        .await?;
    Ok(res)
}

pub async fn list_nodes(pool: &Pool<Postgres>) -> Result<Vec<Node>> {
    let res = sqlx::query_as::<_, Node>(
        "SELECT node_id, node_address, status, last_heartbeat, created_at, updated_at FROM nodes"
    )
        .fetch_all(pool)
        .await?;
    Ok(res)
}

// 注册节点
pub async fn register_node(pool: &Pool<Postgres>, node_address: &str) -> Result<Node> {
    let res = sqlx::query_as::<_, Node>(
        "INSERT INTO nodes (node_address) VALUES ($1)
         RETURNING node_id, node_address, status, last_heartbeat, created_at, updated_at"
    )
        .bind(node_address)
        .fetch_one(pool)
        .await?;
    Ok(res)
}

// 更新节点心跳
pub async fn update_node_heartbeat(pool: &Pool<Postgres>, node_id: Uuid) -> Result<()> {
    let now = Utc::now().naive_utc();
    sqlx::query("UPDATE nodes SET last_heartbeat = $1, updated_at = $1 WHERE node_id = $2")
        .bind(now)
        .bind(node_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_all_active_nodes(pool: &Pool<Postgres>) -> Result<Vec<Node>> {
    let res = sqlx::query_as::<_, Node>(
        "SELECT node_id, node_address, status, last_heartbeat, created_at, updated_at
         FROM nodes WHERE status = 'active'"
    )
        .fetch_all(pool)
        .await?;
    Ok(res)
}

pub async fn insert_chunk_node(pool: &Pool<Postgres>, chunk_id: Uuid, node_id: Uuid, replica_index: i32) -> Result<ChunkNode> {
    let res = sqlx::query_as::<_, ChunkNode>(
        "INSERT INTO chunk_nodes (chunk_id, node_id, replica_index, status) VALUES ($1, $2, $3, 'available')
         RETURNING chunk_id, node_id, replica_index, status, updated_at"
    )
        .bind(chunk_id)
        .bind(node_id)
        .bind(replica_index)
        .fetch_one(pool)
        .await?;
    Ok(res)
}

pub async fn get_chunks_by_file_id(pool: &Pool<Postgres>, file_id: Uuid) -> Result<Vec<FileChunk>> {
    let res = sqlx::query_as::<_, FileChunk>(
        "SELECT chunk_id, file_id, chunk_index, checksum, size, created_at
         FROM file_chunks WHERE file_id = $1 ORDER BY chunk_index ASC"
    )
        .bind(file_id)
        .fetch_all(pool)
        .await?;
    Ok(res)
}

pub async fn get_nodes_for_chunk(pool: &Pool<Postgres>, chunk_id: Uuid) -> Result<Vec<(Uuid, String)>> {
    let res = sqlx::query(
        "SELECT cn.node_id, n.node_address
         FROM chunk_nodes cn
         JOIN nodes n ON cn.node_id = n.node_id
         WHERE cn.chunk_id = $1 AND cn.status = 'available'"
    )
        .bind(chunk_id)
        .fetch_all(pool)
        .await?;

    let mut nodes = Vec::new();
    for row in res {
        let node_id: Uuid = row.try_get("node_id")?;
        let node_address: String = row.try_get("node_address")?;
        nodes.push((node_id, node_address));
    }

    Ok(nodes)
}
