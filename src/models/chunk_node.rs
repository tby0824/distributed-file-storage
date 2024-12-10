use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Debug)]
pub struct ChunkNode {
    pub chunk_id: Uuid,
    pub node_id: Uuid,
    pub replica_index: i32,
    pub status: Option<String>,
    pub updated_at: DateTime<Utc>,
}
