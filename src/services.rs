use uuid::Uuid;
use anyhow::Result;
use sqlx::{Pool, Postgres};
use crate::db::queries;
use rand::seq::SliceRandom;
use rand::thread_rng;


const REPLICA_COUNT: usize = 3;

pub async fn select_nodes_for_chunk(pool: &Pool<Postgres>) -> Result<Vec<Uuid>> {
    let nodes = queries::get_all_active_nodes(pool).await?;
    if nodes.len() < REPLICA_COUNT {

    }

    let mut rng = thread_rng();
    let mut node_ids: Vec<Uuid> = nodes.iter().map(|n| n.node_id).collect();
    node_ids.shuffle(&mut rng);
    Ok(node_ids.into_iter().take(REPLICA_COUNT).collect())
}

pub async fn fetch_chunk_from_nodes(
    nodes: &[(Uuid, String)],
    chunk: &crate::models::FileChunk,
) -> Result<Vec<u8>> {
    for (_node_id, node_address) in nodes {
        let url = format!("{}/get_chunk/{}", node_address, chunk.chunk_id);

        // 请求节点服务获取块数据
        match reqwest::get(&url).await {
            Ok(response) if response.status().is_success() => {
                let data = response.bytes().await?;
                return Ok(data.to_vec());
            }
            Ok(_) => {
                eprintln!("Node at {} returned non-successful status", node_address);
            }
            Err(e) => {
                eprintln!("Failed to fetch from node at {}: {:?}", node_address, e);
            }
        }
    }

    Err(anyhow::anyhow!("Failed to fetch chunk from all nodes"))
}
