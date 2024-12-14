use libp2p::{
    NetworkBehaviour,
    core::ProtocolName,
    request_response::{
        RequestResponse, RequestResponseConfig, RequestResponseEvent, RequestResponseMessage,
        ProtocolSupport, RequestResponseCodec,
    },
};
use futures::prelude::*;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use std::io;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use crate::handlers;
use anyhow::Result;

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

// Add new requests and responses

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadRequest {
    pub owner_id: Uuid,
    pub file_name: String,
    pub file_data: Vec<u8>,
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
pub struct SetPermissionRequest {
    pub user_id: Uuid,
    pub file_name: String,
    pub target_username: String,
    pub can_read: bool,
    pub can_write: bool,
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
pub enum FileRequest {
    Upload(UploadRequest),
    Download(DownloadRequest),
    ListNodes,
    Register(RegisterRequest),
    DeleteFile(DeleteFileRequest),
    BatchUpload(BatchUploadRequest),
    BatchDelete(BatchDeleteRequest),
    ListFiles(ListFilesRequest),
    RenameFile(RenameFileRequest),
    SetPermission(SetPermissionRequest),
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
    pub files: Vec<(String,i64)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenameFileResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetPermissionResponse {
    pub success: bool,
    pub message: String,
}

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
    SetPermission(SetPermissionResponse),
    Error(String),
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "FileStorageProtocolEvent")]
pub struct FileStorageProtocol {
    request_response: RequestResponse<FileCodec>,
    #[behaviour(ignore)]
    pub(crate) db: Pool<Postgres>,
}

#[derive(Debug)]
pub enum FileStorageProtocolEvent {
    FileReceived {
        file_id: Uuid,
        from: libp2p::PeerId,
    },
    RequestFailed {
        peer: libp2p::PeerId,
        error: String,
    },
    ResponseReceived {
        peer: libp2p::PeerId,
        response: FileResponse,
    },
}

impl From<RequestResponseEvent<FileRequest, FileResponse>> for FileStorageProtocolEvent {
    fn from(event: RequestResponseEvent<FileRequest, FileResponse>) -> Self {
        match event {
            RequestResponseEvent::Message { peer, message } => {
                match message {
                    RequestResponseMessage::Response { response, .. } => {
                        FileStorageProtocolEvent::ResponseReceived {
                            peer,
                            response,
                        }
                    }
                    RequestResponseMessage::Request { .. } => {
                        FileStorageProtocolEvent::RequestFailed {
                            peer,
                            error: "Unexpected request".to_string(),
                        }
                    }
                }
            }
            RequestResponseEvent::OutboundFailure { peer, error, .. } => {
                FileStorageProtocolEvent::RequestFailed {
                    peer,
                    error: error.to_string(),
                }
            }
            RequestResponseEvent::InboundFailure { peer, error, .. } => {
                FileStorageProtocolEvent::RequestFailed {
                    peer,
                    error: error.to_string(),
                }
            }
            _ => FileStorageProtocolEvent::RequestFailed {
                peer: libp2p::PeerId::random(),
                error: "Unknown event".to_string(),
            },
        }
    }
}

impl FileStorageProtocol {
    pub fn new(db: Pool<Postgres>) -> Self {
        let protocols = vec![(FileProtocol, ProtocolSupport::Full)];
        let mut cfg = RequestResponseConfig::default();
        cfg.set_request_timeout(Duration::from_secs(60));

        Self {
            request_response: RequestResponse::new(FileCodec, protocols, cfg),
            db,
        }
    }

    pub async fn handle_request(&self, request: FileRequest) -> FileResponse {
        match request {
            FileRequest::Upload(upload_req) => {
                match handlers::handle_upload(&self.db, upload_req).await {
                    Ok(file_id) => FileResponse::Upload(UploadResponse { file_id }),
                    Err(e) => FileResponse::Error(e.to_string()),
                }
            }
            FileRequest::Download(download_req) => {
                match handlers::handle_download(&self.db, download_req).await {
                    Ok((data, name)) => FileResponse::Download(DownloadResponse {
                        file_data: data,
                        file_name: name,
                    }),
                    Err(e) => FileResponse::Error(e.to_string()),
                }
            }
            FileRequest::ListNodes => {
                match handlers::handle_list_nodes(&self.db).await {
                    Ok(nodes) => FileResponse::ListNodes(ListNodesResponse { nodes }),
                    Err(e) => FileResponse::Error(e.to_string()),
                }
            }
            FileRequest::Register(register_req) => {
                match handlers::handle_register(&self.db, register_req).await {
                    Ok((user_id, username)) => FileResponse::Register(RegisterResponse { user_id, username }),
                    Err(e) => FileResponse::Error(e.to_string()),
                }
            }
            FileRequest::DeleteFile(delete_req) => {
                match handlers::handle_delete_file(&self.db, delete_req).await {
                    Ok(message) => FileResponse::DeleteFile(DeleteFileResponse {
                        success: true,
                        message
                    }),
                    Err(e) => FileResponse::Error(e.to_string()),
                }
            }
            FileRequest::BatchUpload(req) => {
                match handlers::handle_batch_upload(&self.db, req.owner_id, req.file_paths, req.force).await {
                    Ok(file_ids) => FileResponse::BatchUpload(BatchUploadResponse { file_ids }),
                    Err(e) => FileResponse::Error(e.to_string()),
                }
            }
            FileRequest::BatchDelete(req) => {
                match handlers::handle_batch_delete(&self.db, req.user_id, req.file_names).await {
                    Ok(results) => FileResponse::BatchDelete(BatchDeleteResponse { results }),
                    Err(e) => FileResponse::Error(e.to_string()),
                }
            }
            FileRequest::ListFiles(req) => {
                match handlers::handle_list_files(&self.db, req.owner_id).await {
                    Ok(files) => FileResponse::ListFiles(ListFilesResponse { files }),
                    Err(e) => FileResponse::Error(e.to_string()),
                }
            }
            FileRequest::RenameFile(req) => {
                match handlers::handle_rename_file(&self.db, req.user_id, &req.old_name, &req.new_name).await {
                    Ok((success, message)) => FileResponse::RenameFile(RenameFileResponse { success, message }),
                    Err(e) => FileResponse::Error(e.to_string()),
                }
            }
            FileRequest::SetPermission(req) => {
                match handlers::handle_set_permission(&self.db, req.user_id, &req.file_name, &req.target_username, req.can_read, req.can_write).await {
                    Ok((success, message)) => FileResponse::SetPermission(SetPermissionResponse { success, message }),
                    Err(e) => FileResponse::Error(e.to_string()),
                }
            }
        }
    }

    // 为本地请求添加一个异步方法，不需要对等点即可使用
    pub async fn handle_local_request(&self, req: FileRequest) -> FileResponse {
        self.handle_request(req).await
    }

    pub fn send_upload_request(&mut self, peer: &libp2p::PeerId, req: UploadRequest) {
        let request = FileRequest::Upload(req);
        self.request_response.send_request(peer, request);
    }
}
