use crate::network::{UploadRequest, DownloadRequest, NodeInfo, RegisterRequest, DeleteFileRequest};
use sqlx::{Pool, Postgres};
use uuid::Uuid;
use anyhow::{Result, anyhow};
use crate::db::queries;
use tokio::fs;
use std::path::Path;
use argon2::{Argon2, PasswordHasher, PasswordVerifier, password_hash::PasswordHash};
use rand::rngs::OsRng;
use argon2::password_hash::SaltString;
use crate::models::User;
use jsonwebtoken::{encode, Header, Algorithm, EncodingKey};
use serde::{Serialize, Deserialize};

const CHUNK_SIZE: usize = 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    user_id: Uuid,
    exp: usize,
}

pub fn generate_jwt(secret: &str, user: &User) -> Result<String> {
    let expiration = (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize;
    let claims = Claims {
        sub: user.username.clone(),
        user_id: user.user_id,
        exp: expiration,
    };
    let token = encode(&Header::new(Algorithm::HS256), &claims, &EncodingKey::from_secret(secret.as_ref()))?;
    Ok(token)
}

// 如果需要，可实现 validate_jwt
// pub fn validate_jwt(token: &str, secret: &str) -> Result<Claims> {
//     let decoded = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_ref()), &Validation::new(Algorithm::HS256))?;
//     Ok(decoded.claims)
// }

pub async fn handle_upload(pool: &Pool<Postgres>, req: UploadRequest) -> Result<Uuid> {
    if let Some(existing) = queries::get_file_by_name(pool, req.owner_id, &req.file_name).await? {
        if !queries::can_write_file(pool, req.owner_id, existing.file_id).await? {
            return Err(anyhow!("No write permission."));
        }
        queries::delete_file_by_id(pool, existing.file_id).await?;
        let mut i = 0;
        loop {
            let chunk_path = format!("chunks/{}_{}.bin", existing.file_id, i);
            if !Path::new(&chunk_path).exists() {
                break;
            }
            let _ = fs::remove_file(&chunk_path).await;
            i += 1;
        }
    }

    let file_id = queries::insert_file_meta(pool, req.owner_id, &req.file_name, req.file_data.len() as i64).await?;
    fs::create_dir_all("chunks").await?;

    let data = req.file_data;
    let mut offset = 0;
    let mut chunk_index = 0;
    while offset < data.len() {
        let end = (offset + CHUNK_SIZE).min(data.len());
        let chunk = data[offset..end].to_vec();
        let chunk_path = format!("chunks/{}_{}.bin", file_id, chunk_index);
        fs::write(&chunk_path, &chunk).await?;
        offset += CHUNK_SIZE;
        chunk_index += 1;
    }

    Ok(file_id)
}

pub async fn handle_download(pool: &Pool<Postgres>, req: DownloadRequest) -> Result<(Vec<u8>, String)> {
    let file_meta = queries::get_file_by_id(pool, req.file_id).await?
        .ok_or_else(|| anyhow!("File not found"))?;
    if !queries::can_read_file(pool, file_meta.owner_id, req.file_id).await? {
        return Err(anyhow!("No read permission."));
    }
    let mut chunks_data = Vec::new();
    for i in 0.. {
        let chunk_path = format!("chunks/{}_{}.bin", req.file_id, i);
        if !Path::new(&chunk_path).exists() {
            break;
        }
        let chunk = fs::read(&chunk_path).await?;
        chunks_data.extend_from_slice(&chunk);
    }
    Ok((chunks_data, file_meta.file_name))
}

pub async fn handle_list_nodes(_pool: &Pool<Postgres>) -> Result<Vec<NodeInfo>> {
    // 未实现节点列表，这里返回空
    Ok(Vec::new())
}

pub async fn handle_register(pool: &Pool<Postgres>, req: RegisterRequest) -> Result<(Uuid, String)> {
    if let Some(ref email) = req.email {
        if !email.contains('@') || !email.contains('.') {
            return Err(anyhow!("Invalid email format."));
        }
        if queries::get_user_by_email(pool, email).await?.is_some() {
            return Err(anyhow!("Email already in use."));
        }
    }
    if queries::get_user_by_username(pool, &req.username).await?.is_some() {
        return Err(anyhow!("Username already taken."));
    }
    if req.password.len() < 6 {
        return Err(anyhow!("Password too short."));
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(req.password.as_bytes(), &salt)
        .map_err(|e| anyhow!("Failed to hash password: {}", e))?
        .to_string();

    let user_id = queries::insert_user(pool, &req.username, &password_hash, req.email.as_deref()).await?;
    Ok((user_id, req.username))
}

pub async fn handle_delete_file(pool: &Pool<Postgres>, req: DeleteFileRequest) -> Result<String> {
    queries::get_file_by_id(pool, req.file_id).await?
        .ok_or_else(|| anyhow!("File not found"))?;
    if !queries::can_write_file(pool, req.user_id, req.file_id).await? {
        return Err(anyhow!("No write permission."));
    }
    let rows = queries::delete_file_by_id(pool, req.file_id).await?;
    if rows > 0 {
        let mut i = 0;
        loop {
            let chunk_path = format!("chunks/{}_{}.bin", req.file_id, i);
            if !Path::new(&chunk_path).exists() {
                break;
            }
            let _ = fs::remove_file(&chunk_path).await;
            i += 1;
        }
        Ok("File deleted successfully".to_string())
    } else {
        Err(anyhow!("File not found"))
    }
}

pub async fn login_user(pool: &Pool<Postgres>, username: &str, password: &str) -> Result<Option<User>> {
    let user_opt = queries::get_user_by_username(pool, username).await?;
    if let Some(user) = user_opt {
        let parsed_hash = PasswordHash::new(&user.password)
            .map_err(|e| anyhow!("Failed to parse password hash: {}", e))?;
        let argon2 = Argon2::default();
        if argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok() {
            return Ok(Some(user));
        }
    }
    Ok(None)
}


pub async fn handle_batch_upload(pool: &Pool<Postgres>, owner_id: Uuid, file_paths: Vec<String>, force: bool) -> Result<Vec<Uuid>> {
    let mut uploaded_ids = Vec::new();
    for file_path in file_paths {
        if force {
            if let Some(existing) = queries::get_file_by_name(pool, owner_id, &file_path).await? {
                queries::delete_file_by_id(pool, existing.file_id).await?;
            }
        } else {
            if queries::get_file_by_name(pool, owner_id, &file_path).await?.is_some() {
                continue;
            }
        }
        let data = std::fs::read(&file_path)?;
        let file_id = queries::insert_file_meta(pool, owner_id, &file_path, data.len() as i64).await?;
        fs::create_dir_all("chunks").await?;
        let chunk_path = format!("chunks/{}.bin", file_id);
        fs::write(&chunk_path, &data).await?;
        uploaded_ids.push(file_id);
    }
    Ok(uploaded_ids)
}

pub async fn handle_batch_delete(pool: &Pool<Postgres>, user_id: Uuid, file_names: Vec<String>) -> Result<Vec<String>> {
    let mut results = Vec::new();
    for fname in file_names {
        if let Some(file) = queries::get_file_by_name(pool, user_id, &fname).await? {
            queries::delete_file_by_id(pool, file.file_id).await?;
            results.push(format!("{}: deleted", fname));
        } else {
            results.push(format!("{}: no such file or no permission", fname));
        }
    }
    Ok(results)
}

pub async fn handle_list_files(pool: &Pool<Postgres>, owner_id: Uuid) -> Result<Vec<(String,i64)>> {
    let files = queries::list_user_files(pool, owner_id).await?;
    let res = files.into_iter().map(|f| (f.file_name, f.file_size)).collect();
    Ok(res)
}

pub async fn handle_rename_file(pool: &Pool<Postgres>, user_id: Uuid, old_name: &str, new_name: &str) -> Result<(bool,String)> {
    let rows = queries::rename_file(pool, user_id, old_name, new_name).await?;
    if rows > 0 {
        Ok((true, format!("File '{}' renamed to '{}'", old_name, new_name)))
    } else {
        Ok((false, format!("No file named '{}' or no permission.", old_name)))
    }
}
