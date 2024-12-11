use crate::db::queries;
use crate::services::{fetch_chunk_from_nodes,select_nodes_for_chunk};
use anyhow::Result;
use bcrypt::{hash, DEFAULT_COST};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres};
use std::fs::create_dir_all;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
use warp::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use warp::http::Response;
use warp::http::StatusCode;
use warp::reply::json;
use warp::reply::with_status;
use warp::{Rejection, Reply, Filter};

// 定义自定义错误类型
#[derive(Debug)]
struct Unauthorized;

impl warp::reject::Reject for Unauthorized {}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

pub fn get_jwt_secret() -> String {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string())
}

pub async fn register_user(pool: Pool<Postgres>, req: RegisterRequest) -> Result<impl Reply, Rejection> {
    // 检查用户名是否已存在
    if let Some(_) = queries::get_user_by_username(&pool, &req.username)
        .await
        .map_err(|_| warp::reject())?
    {
        return Ok(with_status(
            json(&json!({ "status": "error", "message": "Username already exists" })),
            StatusCode::CONFLICT,
        ));
    }

    let hashed = hash(&req.password, DEFAULT_COST).map_err(|_| warp::reject())?;
    let user = queries::create_user(&pool, &req.username, &hashed, req.email.as_deref())
        .await.map_err(|_| warp::reject())?;

    let expiration = Utc::now().timestamp() as usize + 3600;
    let claims = Claims { sub: user.user_id.to_string(), exp: expiration };
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(get_jwt_secret().as_ref())).unwrap();

    Ok(with_status(
        json(&json!({ "user": user, "token": token })),
        StatusCode::OK,
    ))
}



pub fn with_auth() -> impl Filter<Extract = ((),), Error = Rejection> + Clone {
    warp::header::<String>("authorization")
        .and_then(|auth_header: String| async move {
            let token = auth_header.trim_start_matches("Bearer ").to_string();
            match decode::<Claims>(
                &token,
                &DecodingKey::from_secret(get_jwt_secret().as_ref()),
                &Validation::default(),
            ) {
                Ok(_) => Ok(()),
                Err(_) => Err(warp::reject::custom(Unauthorized)),
            }
        })
}

// 上传文件请求结构体
#[derive(Deserialize)]
pub struct UploadRequest {
    pub owner_id: Uuid,
    pub file_name: String,
    pub file_path: String,
    pub chunk_size: usize,
}

pub async fn upload_file(pool: Pool<Postgres>, req: UploadRequest) -> Result<impl Reply, Rejection> {
    let result = async {
        let mut file = File::open(&req.file_path).await?;
        let metadata = file.metadata().await?;
        let file_size = metadata.len() as i64;

        let file_meta = queries::create_file_metadata(&pool, req.owner_id, &req.file_name, file_size)
            .await?;

        create_dir_all("chunks")?;

        let mut index = 0;
        let mut buf = vec![0; req.chunk_size];

        loop {
            let bytes_read = file.read(&mut buf).await?;
            if bytes_read == 0 {
                break;
            }

            let chunk_data = &buf[..bytes_read];
            let checksum = format!("{:x}", md5::compute(chunk_data));

            let chunk_record = queries::create_file_chunk(
                &pool,
                file_meta.file_id,
                index,
                &checksum,
                bytes_read as i64,
            )
                .await?;

            // 本地暂存chunk
            let chunk_path = format!("chunks/{}.bin", chunk_record.chunk_id);
            let mut chunk_file = File::create(&chunk_path).await?;
            chunk_file.write_all(chunk_data).await?;

            // 为chunk选择存储节点
            let replica_nodes = select_nodes_for_chunk(&pool).await?;
            for (replica_index, node_id) in replica_nodes.into_iter().enumerate() {
                queries::insert_chunk_node(&pool, chunk_record.chunk_id, node_id, replica_index as i32)
                    .await?;
            }

            index += 1;
        }

        Ok(file_meta.file_id) as Result<Uuid, Box<dyn std::error::Error>>
    }
        .await;

    match result {
        Ok(file_id) => Ok(with_status(
            json(&json!({
            "status": "success",
            "file_id": file_id
        })),
            StatusCode::OK,
        )),
        Err(_) => Ok(with_status(
            json(&json!({
            "status": "error",
            "message": "Failed to upload file"
        })),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }

}
pub async fn heartbeat_node(pool: Pool<Postgres>, node_id: Uuid) -> Result<impl Reply, Rejection> {
    queries::update_node_heartbeat(&pool, node_id)
        .await
        .map_err(|_| warp::reject())?;

    Ok(with_status(
        json(&json!({ "status": "success", "message": "Heartbeat updated" })),
        StatusCode::OK,
    ))
}


pub async fn list_all_nodes(pool: Pool<Postgres>) -> Result<impl Reply, Rejection> {
    let nodes = queries::list_nodes(&pool).await.map_err(|_| warp::reject())?;
    Ok(json(&nodes))
}

#[derive(Deserialize)]
pub struct NodeRegisterRequest {
    pub node_address: String,
}

pub async fn register_node(pool: Pool<Postgres>, req: NodeRegisterRequest) -> Result<impl Reply, Rejection> {
    let node = queries::register_node(&pool, &req.node_address).await.map_err(|_| warp::reject())?;
    Ok(json(&node))
}

pub async fn download_file(pool: Pool<Postgres>, file_id: Uuid) -> Result<impl Reply, Rejection> {
    let file_meta = sqlx::query_as::<_, crate::models::FileMeta>(
        "SELECT file_id, owner_id, file_name, file_size, created_at, updated_at FROM files WHERE file_id = $1",
    )
        .bind(file_id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| warp::reject())?
        .ok_or_else(warp::reject)?;

    let chunks = queries::get_chunks_by_file_id(&pool, file_id)
        .await
        .map_err(|_| warp::reject())?;

    let mut file_data = Vec::with_capacity(file_meta.file_size as usize);
    for chunk in chunks {
        let nodes_for_chunk = queries::get_nodes_for_chunk(&pool, chunk.chunk_id)
            .await
            .map_err(|_| warp::reject())?;

        let chunk_data = fetch_chunk_from_nodes(&nodes_for_chunk, &chunk)
            .await
            .map_err(|e| {
                eprintln!("Failed to fetch chunk {}: {:?}", chunk.chunk_id, e);
                warp::reject()
            })?;

        if chunk_data.is_empty() {
            eprintln!("Chunk {} returned empty data", chunk.chunk_id);
        }

        file_data.extend_from_slice(&chunk_data);
    }

    let resp = Response::builder()
        .header(CONTENT_TYPE, "application/octet-stream")
        .header(
            CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file_meta.file_name),
        )
        .body(file_data);

    Ok(resp)
}

