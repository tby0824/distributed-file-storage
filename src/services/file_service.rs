use tonic::{Request, Response, Status};
use crate::utils::jwt::{decode_jwt};
use crate::services::auth_service::authenticate;
use crate::db::{files, file_chunks, chunk_nodes, nodes};
use crate::distribution::distribute_chunks;
use sqlx::Pool;
use sqlx::Postgres;
use uuid::Uuid;
use prometheus::{Counter, Histogram};
use crate::node::communication::{CommunicationController, ChunkMessage};
use crate::proto::{AuthenticateRequest, AuthenticateResponse, DeleteFileRequest, DeleteFileResponse, DownloadRequest, DownloadResponse, FileItem, GetFileMetaRequest, GetFileMetaResponse, ListFilesRequest, ListFilesResponse, UploadRequest, UploadResponse};
use crate::proto::file_storage_server::FileStorage;

#[derive(Clone)]
pub struct FileService {
    pub pool: Pool<Postgres>,
    pub jwt_secret: String,
    pub uploaded_files_counter: Counter,
    pub downloaded_files_counter: Counter,
    pub request_histogram: Histogram,
    pub communication: CommunicationController,
}

#[tonic::async_trait]
impl FileStorage for FileService {
    async fn authenticate(&self, request: Request<AuthenticateRequest>)
                          -> Result<Response<AuthenticateResponse>, Status> {
        let start_time = std::time::Instant::now();
        let req = request.into_inner();

        let token = authenticate(&self.pool, &self.jwt_secret, &req.username, &req.password).await?;
        let elapsed = start_time.elapsed().as_secs_f64();
        self.request_histogram.observe(elapsed);

        Ok(Response::new(AuthenticateResponse{
            status: "OK".to_string(),
            jwt_token: token,
        }))
    }

    async fn upload_file(&self, request: Request<UploadRequest>) -> Result<Response<UploadResponse>, Status> {
        let start_time = std::time::Instant::now();
        let req = request.into_inner();

        let claims = decode_jwt(&req.jwt_token, &self.jwt_secret).map_err(|_| Status::unauthenticated("Invalid token"))?;

        let file_data = req.file_data;
        let chunk_size = 1024 * 1024;
        let mut chunks_data = vec![];
        let mut offset = 0;
        while offset < file_data.len() {
            let end = (offset + chunk_size).min(file_data.len());
            let chunk_data = &file_data[offset..end];
            chunks_data.push(chunk_data.to_vec());
            offset = end;
        }

        let file_record = files::create_file(
            &self.pool,
            Some(claims.sub),
            &req.file_name,
            file_data.len() as i64
        ).await.map_err(|_| Status::internal("DB error creating file"))?;

        let node_list = nodes::get_all_nodes(&self.pool).await.map_err(|_| Status::internal("Cannot fetch nodes"))?;
        if node_list.is_empty() {
            return Err(Status::failed_precondition("No nodes available"));
        }

        let replica_count = 3;
        let assignments = distribute_chunks(&chunks_data, &node_list, replica_count);

        for (chunk_index, (chunk_data, node_assignments)) in assignments.into_iter().enumerate() {
            let checksum = format!("{:x}", md5::compute(&chunk_data));
            let chunk_record = file_chunks::create_file_chunk(&self.pool, file_record.file_id, chunk_index as i32, &checksum, chunk_data.len() as i64)
                .await.map_err(|_| Status::internal("DB error creating chunk"))?;

            for (replica_index, node) in node_assignments.into_iter().enumerate() {
                chunk_nodes::assign_chunk_to_node(&self.pool, chunk_record.chunk_id, node.node_id, replica_index as i32)
                    .await.map_err(|_| Status::internal("DB error assigning chunk to node"))?;

                let msg = ChunkMessage::StoreChunk {
                    chunk_id: chunk_record.chunk_id,
                    chunk_data: chunk_data.clone(),
                };
                self.communication.broadcast_message(msg).await;
            }
        }

        self.uploaded_files_counter.inc();
        let elapsed = start_time.elapsed().as_secs_f64();
        self.request_histogram.observe(elapsed);

        Ok(Response::new(UploadResponse{
            status: "OK".to_string(),
            file_id: file_record.file_id.to_string(),
        }))
    }

    async fn download_file(&self, request: Request<DownloadRequest>) -> Result<Response<DownloadResponse>, Status> {
        let start_time = std::time::Instant::now();
        let req = request.into_inner();

        let claims = decode_jwt(&req.jwt_token, &self.jwt_secret).map_err(|_| Status::unauthenticated("Invalid token"))?;
        let file_id = Uuid::parse_str(&req.file_id).map_err(|_| Status::invalid_argument("Invalid file_id"))?;

        let file_record = files::get_file_by_id(&self.pool, file_id).await.map_err(|_| Status::not_found("File not found"))?;
        if file_record.owner_id != Some(claims.sub) && claims.role != "admin" {
            return Err(Status::permission_denied("Not allowed"));
        }

        let file_chunks = file_chunks::get_chunks_by_file_id(&self.pool, file_id).await.map_err(|_| Status::internal("DB error"))?;

        let mut file_data = vec![];

        for c in file_chunks {
            let msg = ChunkMessage::GetChunk { chunk_id: c.chunk_id };
            self.communication.broadcast_message(msg).await;
            if let Some(data) = self.communication.wait_for_response(c.chunk_id).await {
                file_data.extend_from_slice(&data);
            } else {
                log::error!("Failed to retrieve chunk data {} from network", c.chunk_id);
                return Err(Status::internal("Failed to retrieve chunk data"));
            }
        }

        self.downloaded_files_counter.inc();
        let elapsed = start_time.elapsed().as_secs_f64();
        self.request_histogram.observe(elapsed);

        Ok(Response::new(DownloadResponse{
            status: "OK".to_string(),
            file_data,
        }))
    }

    async fn list_files(&self, request: Request<ListFilesRequest>) -> Result<Response<ListFilesResponse>, Status> {
        let start_time = std::time::Instant::now();
        let req = request.into_inner();
        let claims = decode_jwt(&req.jwt_token, &self.jwt_secret).map_err(|_| Status::unauthenticated("Invalid token"))?;

        let user_files = if claims.role == "admin" {
            files::list_all_files(&self.pool).await
        } else {
            files::list_user_files(&self.pool, claims.sub).await
        }.map_err(|_| Status::internal("DB error"))?;

        let items = user_files.into_iter().map(|f| FileItem {
            file_id: f.file_id.to_string(),
            file_name: f.file_name,
            file_size: f.file_size,
        }).collect();

        let elapsed = start_time.elapsed().as_secs_f64();
        self.request_histogram.observe(elapsed);

        Ok(Response::new(ListFilesResponse{
            files: items
        }))
    }

    async fn delete_file(&self, request: Request<DeleteFileRequest>) -> Result<Response<DeleteFileResponse>, Status> {
        let start_time = std::time::Instant::now();
        let req = request.into_inner();
        let claims = decode_jwt(&req.jwt_token, &self.jwt_secret).map_err(|_| Status::unauthenticated("Invalid token"))?;
        let file_id = Uuid::parse_str(&req.file_id).map_err(|_| Status::invalid_argument("Invalid file_id"))?;

        let file_record = files::get_file_by_id(&self.pool, file_id).await.map_err(|_| Status::not_found("File not found"))?;

        if file_record.owner_id != Some(claims.sub) && claims.role != "admin" {
            return Err(Status::permission_denied("Not allowed"));
        }

        files::delete_file(&self.pool, file_id).await.map_err(|_| Status::internal("DB error"))?;

        let elapsed = start_time.elapsed().as_secs_f64();
        self.request_histogram.observe(elapsed);

        Ok(Response::new(DeleteFileResponse{
            status: "OK".to_string()
        }))
    }

    async fn get_file_meta(&self, request: Request<GetFileMetaRequest>) -> Result<Response<GetFileMetaResponse>, Status> {
        let start_time = std::time::Instant::now();
        let req = request.into_inner();
        let claims = decode_jwt(&req.jwt_token, &self.jwt_secret).map_err(|_| Status::unauthenticated("Invalid token"))?;
        let file_id = Uuid::parse_str(&req.file_id).map_err(|_| Status::invalid_argument("Invalid file_id"))?;
        let file_record = files::get_file_by_id(&self.pool, file_id).await.map_err(|_| Status::not_found("File not found"))?;
        if file_record.owner_id != Some(claims.sub) && claims.role != "admin" {
            return Err(Status::permission_denied("Not allowed"));
        }

        let elapsed = start_time.elapsed().as_secs_f64();
        self.request_histogram.observe(elapsed);

        Ok(Response::new(GetFileMetaResponse{
            status: "OK".to_string(),
            file_name: file_record.file_name,
            file_size: file_record.file_size,
        }))
    }
}
