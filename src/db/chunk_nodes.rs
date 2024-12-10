use sqlx::{Pool, Postgres};
use crate::models::chunk_node::ChunkNode;
use uuid::Uuid;

pub async fn assign_chunk_to_node(
    pool: &Pool<Postgres>,
    chunk_id: Uuid,
    node_id: Uuid,
    replica_index: i32
) -> Result<ChunkNode, sqlx::Error> {
    let record = sqlx::query_as!(ChunkNode,
        r#"
        INSERT INTO chunk_nodes (chunk_id, node_id, replica_index)
        VALUES ($1, $2, $3)
        RETURNING chunk_id, node_id, replica_index, status, updated_at
        "#,
        chunk_id, node_id, replica_index
    )
        .fetch_one(pool)
        .await?;
    Ok(record)
}

pub async fn get_nodes_for_chunk(pool: &Pool<Postgres>, chunk_id: Uuid) -> Result<Vec<ChunkNode>, sqlx::Error> {
    let records = sqlx::query_as!(ChunkNode,
        r#"
        SELECT chunk_id, node_id, replica_index, status, updated_at
        FROM chunk_nodes
        WHERE chunk_id = $1
        ORDER BY replica_index
        "#,
        chunk_id
    )
        .fetch_all(pool)
        .await?;
    Ok(records)
}
