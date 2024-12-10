use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub role: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
