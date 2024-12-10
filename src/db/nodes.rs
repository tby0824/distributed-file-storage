use sqlx::{Pool, Postgres};
use crate::models::node::Node;
use uuid::Uuid;
use chrono::Utc;

pub async fn register_node(pool: &Pool<Postgres>, node_address: &str) -> Result<Node, sqlx::Error> {
    let record = sqlx::query_as!(Node,
        r#"
        INSERT INTO nodes (node_address)
        VALUES ($1)
        RETURNING node_id, node_address, status, last_heartbeat, created_at, updated_at
        "#,
        node_address
    )
        .fetch_one(pool)
        .await?;
    Ok(record)
}

pub async fn update_node_heartbeat(pool: &Pool<Postgres>, node_id: Uuid) -> Result<Node, sqlx::Error> {
    let now = Utc::now();
    let record = sqlx::query_as!(Node,
        r#"
        UPDATE nodes
        SET last_heartbeat = $1, updated_at = $1
        WHERE node_id = $2
        RETURNING node_id, node_address, status, last_heartbeat, created_at, updated_at
        "#,
        now, node_id
    )
        .fetch_one(pool)
        .await?;
    Ok(record)
}

pub async fn get_all_nodes(pool: &Pool<Postgres>) -> Result<Vec<Node>, sqlx::Error> {
    let records = sqlx::query_as!(Node,
        r#"
        SELECT node_id, node_address, status, last_heartbeat, created_at, updated_at
        FROM nodes
        ORDER BY created_at DESC
        "#,
    )
        .fetch_all(pool)
        .await?;
    Ok(records)
}
