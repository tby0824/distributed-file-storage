use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Debug)]
pub struct FileChunk {
    pub chunk_id: Uuid,
    pub file_id: Uuid,
    pub chunk_index: i32,
    pub checksum: String,
    pub size: i64,
    pub created_at: DateTime<Utc>,
}
