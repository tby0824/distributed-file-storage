use libp2p::{
    NetworkBehaviour,
    core::ProtocolName,
    request_response::{
        RequestResponse, RequestResponseConfig, RequestResponseEvent, RequestResponseMessage,
        ProtocolSupport, RequestResponseCodec,
    },
};
use futures::prelude::*;
use log::{error, info};
use sqlx::{Pool, Postgres};
use std::time::Duration;
use std::io;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use crate::handlers::{
    handle_upload, handle_download, handle_list_nodes, handle_register,
    handle_delete_file, handle_batch_upload, handle_batch_delete, handle_list_files,
    handle_rename_file, handle_register_node, handle_node_heartbeat,
    check_auth,
};
use anyhow::anyhow;

#[derive(Debug, Clone)]
pub struct FileProtocol;

impl ProtocolName for FileProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/file-storage/1.0.0".as_bytes()
    }
}

#[derive(Debug, Clone)]
pub struct FileCodec;

#[async_trait]
impl RequestResponseCodec for FileCodec {
    type Protocol = FileProtocol;
    type Request = FileRequest;
    type Response = FileResponse;


    /// Reads a request from the network.
    async fn read_request<T>(&mut self, _: &FileProtocol, io: &mut T) -> io::Result<Self::Request>
    where T: AsyncRead + Unpin + Send {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        bincode::deserialize(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(&mut self, _: &FileProtocol, io: &mut T) -> io::Result<Self::Response>
    where T: AsyncRead + Unpin + Send {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        bincode::deserialize(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Writes a request to the network.
    async fn write_request<T>(&mut self, _: &FileProtocol, io: &mut T, req: Self::Request) -> io::Result<()>
    where T: AsyncWrite + Unpin + Send {
        let buf = bincode::serialize(&req)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        io.write_all(&buf).await?;
        io.flush().await?;
        Ok(())
    }

    async fn write_response<T>(&mut self, _: &FileProtocol, io: &mut T, res: Self::Response) -> io::Result<()>
    where T: AsyncWrite + Unpin + Send {
        let buf = bincode::serialize(&res)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        io.write_all(&buf).await?;
        io.flush().await?;
        Ok(())
    }
}

// Requests
#[derive(Debug, Serialize, Deserialize)]
pub struct UploadRequest {
    pub owner_id: Uuid,
    pub file_name: String,
    pub file_data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub file_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteFileRequest {
    pub file_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchUploadRequest {
    pub owner_id: Uuid,
    pub file_paths: Vec<String>,
    pub force: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchDeleteRequest {
    pub user_id: Uuid,
    pub file_names: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListFilesRequest {
    pub owner_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenameFileRequest {
    pub user_id: Uuid,
    pub old_name: String,
    pub new_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadChunkRequest {
    pub chunk_id: Uuid,
}


/// Enum representing different types of file requests.
#[derive(Debug, Serialize, Deserialize)]
pub enum FileRequest {
    Upload(UploadRequest, Option<String>),
    Download(DownloadRequest, Option<String>),
    ListNodes(Option<String>),
    Register(RegisterRequest, Option<String>),
    DeleteFile(DeleteFileRequest, Option<String>),
    BatchUpload(BatchUploadRequest, Option<String>),
    BatchDelete(BatchDeleteRequest, Option<String>),
    ListFiles(ListFilesRequest, Option<String>),
    RenameFile(RenameFileRequest, Option<String>),
    RegisterNode(String, Option<String>),
    DownloadChunk(DownloadChunkRequest, Option<String>),
    NodeHeartbeat(Uuid, Option<String>),
}

impl FileRequest {
    pub fn jwt(&self) -> Option<&str> {
        match self {
            FileRequest::Upload(_, jwt)
            | FileRequest::Download(_, jwt)
            | FileRequest::ListNodes(jwt)
            | FileRequest::Register(_, jwt)
            | FileRequest::DeleteFile(_, jwt)
            | FileRequest::BatchUpload(_, jwt)
            | FileRequest::BatchDelete(_, jwt)
            | FileRequest::ListFiles(_, jwt)
            | FileRequest::RenameFile(_, jwt)
            | FileRequest::RegisterNode(_, jwt)
            | FileRequest::DownloadChunk(_, jwt)
            | FileRequest::NodeHeartbeat(_, jwt) => jwt.as_deref(),
        }
    }
}

// Responses
#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub user_id: Uuid,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteFileResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadResponse {
    pub file_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadResponse {
    pub file_data: Vec<u8>,
    pub file_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListNodesResponse {
    pub nodes: Vec<NodeInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchUploadResponse {
    pub file_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchDeleteResponse {
    pub results: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListFilesResponse {
    pub files: Vec<FileListItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileListItem {
    pub file_id: Uuid,
    pub file_name: String,
    pub file_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenameFileResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterNodeResponse {
    pub node_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadChunkResponse {
    pub chunk_data: Vec<u8>,
}


/// Enum representing different types of file responses.
#[derive(Debug, Serialize, Deserialize)]
pub enum FileResponse {
    Upload(UploadResponse),
    Download(DownloadResponse),
    ListNodes(ListNodesResponse),
    Register(RegisterResponse),
    DeleteFile(DeleteFileResponse),
    BatchUpload(BatchUploadResponse),
    BatchDelete(BatchDeleteResponse),
    ListFiles(ListFilesResponse),
    RenameFile(RenameFileResponse),
    RegisterNode(RegisterNodeResponse),
    DownloadChunk(DownloadChunkResponse),
    NodeHeartbeatOk,
    Error(String),
}


/// Custom network behaviour integrating the request-response protocol.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "FileStorageProtocolEvent")]
pub struct FileStorageProtocol {
    request_response: RequestResponse<FileCodec>,
    #[behaviour(ignore)]
    pub(crate) db: Pool<Postgres>,
    #[behaviour(ignore)]
    pub(crate) jwt_secret: String,
}



/// Events emitted by the FileStorageProtocol behaviour.
#[derive(Debug)]
pub enum FileStorageProtocolEvent {
    RequestFailed {
        peer: libp2p::PeerId,
        error: String,
    },
    ResponseReceived {
        peer: libp2p::PeerId,
        response: FileResponse,
    },
}
impl FileStorageProtocolEvent {
    #[allow(dead_code)]
    fn debug_info(&self) {
        match self {
            FileStorageProtocolEvent::RequestFailed { peer, error } => {
                let _ = (peer, error);
            }
            FileStorageProtocolEvent::ResponseReceived { peer, response: _ } => {
                let _ = peer;
            }
        }
    }
}
impl From<RequestResponseEvent<FileRequest, FileResponse>> for FileStorageProtocolEvent {
    fn from(event: RequestResponseEvent<FileRequest, FileResponse>) -> Self {
        match event {
            RequestResponseEvent::Message { peer, message } => {
                match message {
                    RequestResponseMessage::Response { response, .. } => {
                        info!("Received response from peer: {}", peer);
                        FileStorageProtocolEvent::ResponseReceived {
                            peer,
                            response,
                        }
                    }
                    RequestResponseMessage::Request { .. } => {
                        error!("Unexpected request from peer: {}", peer);
                        FileStorageProtocolEvent::RequestFailed {
                            peer,
                            error: "Unexpected request".to_string(),
                        }
                    }
                }
            }
            RequestResponseEvent::OutboundFailure { peer, error, .. } => {
                error!("Outbound failure from peer {}: {}", peer, error);
                FileStorageProtocolEvent::RequestFailed {
                    peer,
                    error: error.to_string(),
                }
            }
            RequestResponseEvent::InboundFailure { peer, error, .. } => {
                error!("Inbound failure from peer {}: {}", peer, error);
                FileStorageProtocolEvent::RequestFailed {
                    peer,
                    error: error.to_string(),
                }
            }
            _ => {
                let peer = libp2p::PeerId::random();
                
                error!("Unknown event from peer: {}", peer);
                FileStorageProtocolEvent::RequestFailed {
                    peer,
                    error: "Unknown event".to_string(),
                }
            },
        }
    }
}
unsafe impl Send for FileStorageProtocol {}
unsafe impl Sync for FileStorageProtocol {}

impl FileStorageProtocol {

    /// Initializes a new instance of the FileStorageProtocol behaviour.
    pub fn new(db: Pool<Postgres>, jwt_secret: String) -> Self {
        let protocols = vec![(FileProtocol, ProtocolSupport::Full)];
        let mut cfg = RequestResponseConfig::default();
        cfg.set_request_timeout(Duration::from_secs(60));

        Self {
            request_response: RequestResponse::new(FileCodec, protocols, cfg),
            db,
            jwt_secret,
        }
    }

    /// Handles an incoming request and generates an appropriate response.
    async fn handle_request_ref(&self, request: FileRequest, _local: bool) -> FileResponse {
        if let Err(e) = (|| {
            check_auth(&request, &self.jwt_secret)
        })() {
            return FileResponse::Error(format!("JWT Auth failed: {}", e));
        }

        let result = match request {
            FileRequest::Upload(req, _) => {
                handle_upload(&self.db, req).await
                    .map(|file_id| FileResponse::Upload(UploadResponse { file_id }))
            }
            FileRequest::Download(req, _) => {
                handle_download(&self.db, req).await
                    .map(|(data,name)| FileResponse::Download(DownloadResponse { file_data: data, file_name: name }))
            }
            FileRequest::ListNodes(_) => {
                handle_list_nodes(&self.db).await
                    .map(|nodes| FileResponse::ListNodes(ListNodesResponse { nodes }))
            }
            FileRequest::Register(req, _) => {
                handle_register(&self.db, req).await
                    .map(|(user_id,username)| FileResponse::Register(RegisterResponse { user_id, username }))
            }
            FileRequest::DeleteFile(req, _) => {
                handle_delete_file(&self.db, req).await
                    .map(|message| FileResponse::DeleteFile(DeleteFileResponse { success: true, message }))
            }
            FileRequest::BatchUpload(req, _) => {
                handle_batch_upload(&self.db, req.owner_id, req.file_paths, req.force).await
                    .map(|file_ids| FileResponse::BatchUpload(BatchUploadResponse { file_ids }))
            }
            FileRequest::BatchDelete(req, _) => {
                handle_batch_delete(&self.db, req.user_id, req.file_names).await
                    .map(|results| FileResponse::BatchDelete(BatchDeleteResponse { results }))
            }
            FileRequest::ListFiles(req, _) => {
                handle_list_files(&self.db, req.owner_id).await
                    .map(|files| FileResponse::ListFiles(ListFilesResponse { files }))
            }
            FileRequest::RenameFile(req, _) => {
                handle_rename_file(&self.db, req.user_id, &req.old_name, &req.new_name).await
                    .map(|(success,message)| FileResponse::RenameFile(RenameFileResponse { success, message }))
            }
            FileRequest::RegisterNode(node_address, _) => {
                handle_register_node(&self.db, node_address).await
                    .map(|node_id| FileResponse::RegisterNode(RegisterNodeResponse { node_id }))
            }
            FileRequest::DownloadChunk(_req, _) => {
                Err(anyhow!("DownloadChunk not implemented"))
            }
            FileRequest::NodeHeartbeat(node_id, _) => {
                handle_node_heartbeat(&self.db, node_id).await
                    .map(|_| FileResponse::NodeHeartbeatOk)
            }
        };

        // Return the response or an error if handling failed
        result.unwrap_or_else(|e| FileResponse::Error(format!("Error: {}", e)))
    }


    /// Public method to handle local requests synchronously.
    pub async fn handle_local_request(&self, req: FileRequest) -> FileResponse {
        self.handle_request_ref(req, true).await
    }

    
}
