use crate::models::node::Node;
use hashring::HashRing;

pub fn distribute_chunks(chunks: &Vec<Vec<u8>>, nodes: &Vec<Node>, replica_count: usize) -> Vec<(Vec<u8>, Vec<Node>)> {
    let node_addrs: Vec<String> = nodes.iter().map(|n| n.node_address.clone()).collect();
    let ring = HashRing::new();
    let mut result = vec![];
    for chunk_data in chunks {
        let key = format!("{:x}", md5::compute(chunk_data));
        let mut assigned_nodes = vec![];
        for i in 0..replica_count {
            let node_addr = ring.get_node(&format!("{}-{}", key, i)).expect("No node found");
            let node = nodes.iter().find(|n| n.node_address == node_addr).unwrap();
            assigned_nodes.push(node.clone());
        }
        result.push((chunk_data.clone(), assigned_nodes));
    }
    result
}
