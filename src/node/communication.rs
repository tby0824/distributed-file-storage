use std::collections::HashMap;
use uuid::Uuid;
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Write, Read};
use log::{info, error};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChunkMessage {
    StoreChunk { chunk_id: Uuid, chunk_data: Vec<u8> },
    GetChunk { chunk_id: Uuid },
    ChunkResponse { chunk_id: Uuid, chunk_data: Vec<u8> },
}

pub struct NodeStorage {
    base_dir: PathBuf,
}

impl NodeStorage {
    pub fn new(base_dir: &str) -> Self {
        let path = Path::new(base_dir);
        if !path.exists() {
            fs::create_dir_all(path).expect("Failed to create data directory");
        }
        Self {
            base_dir: path.to_path_buf(),
        }
    }

    pub fn store_chunk(&self, chunk_id: Uuid, chunk_data: &[u8]) -> std::io::Result<()> {
        let file_path = self.base_dir.join(chunk_id.to_string());
        let mut file = File::create(file_path)?;
        file.write_all(chunk_data)?;
        Ok(())
    }

    pub fn get_chunk(&self, chunk_id: &Uuid) -> Option<Vec<u8>> {
        let file_path = self.base_dir.join(chunk_id.to_string());
        if file_path.exists() {
            let mut file = File::open(file_path).ok()?;
            let mut data = vec![];
            file.read_to_end(&mut data).ok()?;
            Some(data)
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct CommunicationController {
    pub request_sender: mpsc::Sender<ChunkMessage>,
    pub response_receiver: ArcMutexReceiver,
}

use std::sync::{Arc, Mutex};
#[derive(Clone)]
pub struct ArcMutexReceiver {
    inner: Arc<Mutex<mpsc::Receiver<ChunkMessage>>>,
}

impl ArcMutexReceiver {
    pub fn new(rx: mpsc::Receiver<ChunkMessage>) -> Self {
        Self { inner: Arc::new(Mutex::new(rx)) }
    }

    pub async fn recv(&self) -> Option<ChunkMessage> {
        let mut rx = self.inner.lock().unwrap();
        rx.recv().await
    }
}

impl CommunicationController {
    pub async fn broadcast_message(&self, msg: ChunkMessage) {
        if let Err(e) = self.request_sender.send(msg).await {
            error!("Failed to send chunk message: {:?}", e);
        }
    }

    pub async fn wait_for_response(&self, chunk_id: Uuid) -> Option<Vec<u8>> {
        loop {
            if let Some(resp) = self.response_receiver.recv().await {
                if let ChunkMessage::ChunkResponse { chunk_id: cid, chunk_data } = resp {
                    if cid == chunk_id {
                        return Some(chunk_data);
                    }
                }
            } else {
                return None;
            }
        }
    }
}
