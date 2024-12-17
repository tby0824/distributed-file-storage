use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::NaiveDateTime;

#[derive(sqlx::FromRow, Serialize, Deserialize)]
pub struct User {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub role: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(sqlx::FromRow, Serialize, Deserialize)]
pub struct FileMeta {
    pub file_id: Uuid,
    pub owner_id: Uuid,
    pub file_name: String,
    pub file_size: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(sqlx::FromRow, Serialize, Deserialize)]
pub struct Node {
    pub node_id: Uuid,
    pub node_address: String,
    pub status: String,
    pub last_heartbeat: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(sqlx::FromRow, Serialize, Deserialize)]
pub struct FileChunk {
    pub chunk_id: Uuid,
    pub file_id: Uuid,
    pub chunk_index: i32,
    pub checksum: String,
    pub size: i64,
    pub created_at: NaiveDateTime,
}

#[derive(sqlx::FromRow, Serialize, Deserialize)]
pub struct ChunkNode {
    pub chunk_id: Uuid,
    pub node_id: Uuid,
    pub replica_index: i32,
    pub status: String,
    pub updated_at: NaiveDateTime,
}
