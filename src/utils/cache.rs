use anyhow::Result;
use redis::{Client, aio::MultiplexedConnection, AsyncCommands};
use serde_json;
use uuid::Uuid;
use crate::models::FileMeta;

#[derive(Clone)]
pub struct Cache {
    client: Client,
}

impl Cache {
    pub fn new(url: &str) -> Self {
        let client = Client::open(url).expect("Failed to create redis client");
        Cache { client }
    }

    async fn get_conn(&self) -> Result<MultiplexedConnection> {
        let conn = self.client.get_multiplexed_async_connection().await?;
        Ok(conn)
    }

    pub async fn get_file_list(&self, user_id: Uuid) -> Result<Option<Vec<FileMeta>>> {
        let mut conn = self.get_conn().await?;
        let key = format!("user:{}:files", user_id);
        let data: Option<String> = conn.get(&key).await?;
        if let Some(json) = data {
            let files: Vec<FileMeta> = serde_json::from_str(&json)?;
            Ok(Some(files))
        } else {
            Ok(None)
        }
    }

    pub async fn set_file_list(&self, user_id: Uuid, files: &Vec<FileMeta>) -> Result<()> {
        let mut conn = self.get_conn().await?;
        let key = format!("user:{}:files", user_id);
        let json = serde_json::to_string(files)?;
        let _: () = conn.set_ex(key, json, 60).await?;
        Ok(())
    }

    pub async fn invalidate_file_list(&self, user_id: Uuid) -> Result<()> {
        let mut conn = self.get_conn().await?;
        let key = format!("user:{}:files", user_id);
        let _: () = conn.del(key).await?;
        Ok(())
    }
}
