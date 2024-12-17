use crate::network::{
    UploadRequest, DownloadRequest, NodeInfo, RegisterRequest, DeleteFileRequest,
    FileRequest,FileListItem
};
use sqlx::{Pool, Postgres,Transaction};
use uuid::Uuid;
use anyhow::{Result, anyhow, Context};
use crate::db::queries;
use tokio::fs;
use std::path::Path;
use argon2::{Argon2, PasswordHasher, PasswordVerifier, password_hash::PasswordHash};
use rand::rngs::OsRng;
use argon2::password_hash::SaltString;
use crate::models::User;
use jsonwebtoken::{encode, decode, Header, Algorithm, EncodingKey, DecodingKey, Validation};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use chrono::{Utc, Duration};
use log::{info, warn};
use std::env;

const CHUNK_SIZE: usize = 1024 * 1024; // 1MB
const NODE_INACTIVE_THRESHOLD_SECS: i64 = 120; // 5分钟不心跳则标记为inactive


/// JWT claims structure for authentication.
#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    user_id: Uuid,
    exp: usize,
}

/// Generates a JWT token for the authenticated user.
pub fn generate_jwt(secret: &str, user: &User) -> Result<String> {
    let expiration = (Utc::now() + chrono::Duration::hours(24)).timestamp() as usize;
    let claims = Claims {
        sub: user.username.clone(),
        user_id: user.user_id,
        exp: expiration,
    };
    let token = encode(&Header::new(Algorithm::HS256), &claims, &EncodingKey::from_secret(secret.as_ref()))
        .context("Failed to generate JWT")?;
    Ok(token)
}

pub fn validate_jwt(token: &str, secret: &str) -> Result<Claims> {
    let decoded = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::new(Algorithm::HS256)
    ).context("JWT decode failed")?;
    Ok(decoded.claims)
}

/// Checks authentication for a given request.
pub fn check_auth(req: &FileRequest, secret: &str) -> Result<Uuid> {
    let needs_auth = matches!(req,
        FileRequest::Upload(_, _)
        | FileRequest::Download(_, _)
        | FileRequest::DeleteFile(_, _)
        | FileRequest::BatchUpload(_, _)
        | FileRequest::BatchDelete(_, _)
        | FileRequest::ListFiles(_, _)
        | FileRequest::RenameFile(_, _)
    );

    if needs_auth {
        let jwt = req.jwt().ok_or_else(|| anyhow!("No JWT provided"))?;
        let claims = validate_jwt(jwt, secret)?;
        Ok(claims.user_id)
    } else {
        Ok(Uuid::nil())
    }
}

pub async fn handle_upload(pool: &Pool<Postgres>, req: UploadRequest) -> Result<Uuid> {
    let mut tx = pool.begin().await.context("Failed to start transaction")?;

    // Check and clean up existing file if present
    if let Some(existing) = queries::get_file_by_name(pool, req.owner_id, &req.file_name).await? {
        if !queries::can_write_file(pool, req.owner_id, existing.file_id).await? {
            return Err(anyhow!("No write permission."));
        }

        // Delete database entries first
        queries::delete_chunk_nodes_by_file_tx(&mut tx, existing.file_id)
            .await
            .context("delete chunk nodes entries")?;

        queries::delete_file_chunks_tx(&mut tx, existing.file_id)
            .await
            .context("delete file chunks from database")?;

        queries::delete_file_by_id_tx(&mut tx, existing.file_id)
            .await
            .context("delete old file")?;

        // Then clean up physical files
        let chunk_dir = format!("chunks/{}", existing.file_id);
        if Path::new(&chunk_dir).exists() {
            // Remove files first
            let mut entries = fs::read_dir(&chunk_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if let Err(e) = fs::remove_file(entry.path()).await {
                    warn!("Failed to remove old chunk file: {}, error: {}", entry.path().display(), e);
                }
            }
            // Then try to remove the directory
            if let Err(e) = fs::remove_dir(&chunk_dir).await {
                warn!("Failed to remove old chunk directory: {}, error: {}", chunk_dir, e);
            }
        }
    }

    let file_id = queries::insert_file_meta_tx(&mut tx, req.owner_id, &req.file_name, req.file_data.len() as i64)
        .await
        .context("insert file meta")?;

    // Create directory for chunks
    let chunk_dir = format!("chunks/{}", file_id);
    fs::create_dir_all(&chunk_dir).await.context("create chunks dir")?;

    let data = req.file_data;
    let mut offset = 0;
    let mut chunk_index = 0;

    while offset < data.len() {
        let end = (offset + CHUNK_SIZE).min(data.len());
        let chunk = &data[offset..end];
        let chunk_path = format!("chunks/{}/chunk_{:04}", file_id, chunk_index);
        fs::write(&chunk_path, &chunk).await.context("write chunk to disk")?;

        let mut hasher = Sha256::new();
        hasher.update(&chunk);
        let checksum = format!("{:x}", hasher.finalize());

        let chunk_id = queries::insert_file_chunk_tx(&mut tx, file_id, chunk_index as i32, &checksum, chunk.len() as i64)
            .await
            .context("insert file_chunk")?;

        // Assign chunk to current node
        assign_chunk_to_node(&mut tx, chunk_id, 0)
            .await
            .context("assign chunk to node")?;

        offset += CHUNK_SIZE;
        chunk_index += 1;
    }

    tx.commit().await.context("commit transaction")?;
    run_chunk_cleanup(&pool).await;

    Ok(file_id)
}

pub async fn handle_download(pool: &Pool<Postgres>, req: DownloadRequest) -> Result<(Vec<u8>, String)> {
    let file_meta = queries::get_file_by_id(pool, req.file_id)
        .await?
        .ok_or_else(|| anyhow!("File not found"))?;

    if !queries::can_read_file(pool, file_meta.owner_id, req.file_id).await? {
        return Err(anyhow!("No read permission."));
    }

    let chunks = queries::get_file_chunks(pool, req.file_id).await.context("get file chunks")?;
    let mut file_data = Vec::with_capacity(file_meta.file_size as usize);

    for chunk in chunks {
        let chunk_path = format!("chunks/{}/chunk_{:04}", req.file_id, chunk.chunk_index);
        if !Path::new(&chunk_path).exists() {
            return Err(anyhow!("Chunk file missing: {}", chunk_path));
        }

        let chunk_data = fs::read(&chunk_path).await.context("read chunk file")?;

        // Verify checksum
        let mut hasher = Sha256::new();
        hasher.update(&chunk_data);
        let actual_checksum = format!("{:x}", hasher.finalize());

        if actual_checksum != chunk.checksum {
            return Err(anyhow!("Chunk checksum mismatch for chunk {}", chunk.chunk_index));
        }

        file_data.extend_from_slice(&chunk_data);
    }

    Ok((file_data, file_meta.file_name))
}
pub async fn handle_list_nodes(pool: &Pool<Postgres>) -> Result<Vec<NodeInfo>> {
    let nodes = queries::list_nodes(pool).await.context("list nodes")?;
    let res: Vec<NodeInfo> = nodes.into_iter().map(|n| NodeInfo {
        id: n.node_id,
        name: n.node_address,
    }).collect();
    Ok(res)
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

    let user_id = queries::insert_user(pool, &req.username, &password_hash, req.email.as_deref())
        .await.context("insert user")?;
    Ok((user_id, req.username))
}

pub async fn handle_delete_file(pool: &Pool<Postgres>, req: DeleteFileRequest) -> Result<String> {
    // Check file exists and permissions
    queries::get_file_by_id(pool, req.file_id)
        .await?.ok_or_else(|| anyhow!("File not found"))?;
    if !queries::can_write_file(pool, req.user_id, req.file_id).await? {
        return Err(anyhow!("No write permission."));
    }

    let mut tx = pool.begin().await.context("begin tx")?;

    // First delete chunk_nodes entries
    queries::delete_chunk_nodes_by_file_tx(&mut tx, req.file_id)
        .await.context("delete chunk nodes entries")?;

    // Then delete chunks from database
    queries::delete_file_chunks_tx(&mut tx, req.file_id)
        .await.context("delete file chunks from database")?;

    // Delete physical chunk files
    let chunk_dir = format!("chunks/{}", req.file_id);
    if Path::new(&chunk_dir).exists() {
        fs::remove_dir_all(&chunk_dir)
            .await
            .context("remove chunk directory")?;
    }

    // Finally delete the file metadata
    let rows = queries::delete_file_by_id_tx(&mut tx, req.file_id)
        .await
        .context("delete file metadata")?;

    tx.commit().await.context("commit tx")?;

    if rows > 0 {
        run_chunk_cleanup(&pool).await;
        Ok("File deleted successfully".to_string())
    } else {
        Err(anyhow!("File not found"))
    }
}

pub async fn login_user(pool: &Pool<Postgres>, username: &str, password: &str) -> Result<Option<User>> {
    let user_opt = queries::get_user_by_username(pool, username)
        .await.context("query user by username")?;
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
    let mut tx = pool.begin().await.context("Failed to start transaction")?;

    for file_path in file_paths {
        if let Some(existing) = queries::get_file_by_name(pool, owner_id, &file_path).await? {
            if force {
                queries::delete_file_by_id_tx(&mut tx, existing.file_id).await.context("delete old file")?;

                let chunk_dir = format!("chunks/{}", existing.file_id);
                if Path::new(&chunk_dir).exists() {
                    let _ = fs::remove_dir_all(&chunk_dir).await;
                }
            } else {
                continue;
            }
        }

        let file_data = std::fs::read(&file_path)
            .with_context(|| format!("read file {}", file_path))?;

        let file_id = queries::insert_file_meta_tx(&mut tx, owner_id, &file_path, file_data.len() as i64)
            .await.context("insert file meta")?;

        let chunk_dir = format!("chunks/{}", file_id);
        fs::create_dir_all(&chunk_dir).await.context("create chunks dir")?;

        let mut offset = 0;
        let mut chunk_index = 0;

        while offset < file_data.len() {
            let end = (offset + CHUNK_SIZE).min(file_data.len());
            let chunk = &file_data[offset..end];
            let chunk_path = format!("chunks/{}/chunk_{:04}", file_id, chunk_index);

            fs::write(&chunk_path, chunk).await
                .context("write chunk to disk")?;

            let mut hasher = Sha256::new();
            hasher.update(chunk);
            let checksum = format!("{:x}", hasher.finalize());

            let chunk_id = queries::insert_file_chunk_tx(&mut tx, file_id, chunk_index as i32, &checksum, chunk.len() as i64)
                .await.context("insert file_chunk")?;

            // Assign chunk to current node
            assign_chunk_to_node(&mut tx, chunk_id, 0)
                .await
                .context("assign chunk to node")?;

            offset += CHUNK_SIZE;
            chunk_index += 1;
        }

        uploaded_ids.push(file_id);
    }

    tx.commit().await.context("commit transaction")?;
    run_chunk_cleanup(pool).await;
    Ok(uploaded_ids)
}
pub async fn handle_batch_delete(pool: &Pool<Postgres>, user_id: Uuid, file_names: Vec<String>) -> Result<Vec<String>> {
    let mut results = Vec::new();
    let mut tx = pool.begin().await.context("begin batch delete tx")?;

    for fname in file_names {
        if let Some(file) = queries::get_file_by_name(pool, user_id, &fname).await? {
            if !queries::can_write_file(pool, user_id, file.file_id).await? {
                results.push(format!("{}: no write permission", fname));
                continue;
            }

            // First delete chunk_nodes entries
            match queries::delete_chunk_nodes_by_file_tx(&mut tx, file.file_id).await {
                Ok(_) => {
                    // Then delete chunks from database
                    match queries::delete_file_chunks_tx(&mut tx, file.file_id).await {
                        Ok(_) => {
                            // Delete physical chunk files
                            let chunk_dir = format!("chunks/{}", file.file_id);
                            if Path::new(&chunk_dir).exists() {
                                if let Err(e) = fs::remove_dir_all(&chunk_dir).await {
                                    results.push(format!("{}: failed to delete chunk files: {}", fname, e));
                                    continue;
                                }
                            }

                            // Finally delete file metadata
                            match queries::delete_file_by_id_tx(&mut tx, file.file_id).await {
                                Ok(_) => results.push(format!("{}: deleted successfully", fname)),
                                Err(e) => results.push(format!("{}: failed to delete metadata: {}", fname, e)),
                            }
                        },
                        Err(e) => results.push(format!("{}: failed to delete chunks from database: {}", fname, e)),
                    }
                },
                Err(e) => results.push(format!("{}: failed to delete chunk nodes: {}", fname, e)),
            }
        } else {
            results.push(format!("{}: no such file or no permission", fname));
        }
    }

    // Commit all changes in a single transaction
    match tx.commit().await {
        Ok(_) => {
            run_chunk_cleanup(&pool).await;
        },
        Err(e) => {
            results.push(format!("Transaction commit failed: {}", e));
        }
    }

    Ok(results)
}
pub async fn handle_list_files(pool: &Pool<Postgres>, owner_id: Uuid) -> Result<Vec<FileListItem>> {
    let files = queries::list_user_files(pool, owner_id).await.context("list user files")?;
    let res = files.into_iter()
        .map(|f| FileListItem {
            file_id: f.file_id,
            file_name: f.file_name,
            file_size: f.file_size,
        })
        .collect();
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

pub async fn handle_register_node(pool: &Pool<Postgres>, node_address: String) -> Result<Uuid> {
    let node_id = queries::insert_node(pool, &node_address).await.context("insert node")?;
    Ok(node_id)
}

pub async fn handle_node_heartbeat(pool: &Pool<Postgres>, node_id: Uuid) -> Result<()> {
    queries::update_node_heartbeat(pool, node_id, Utc::now()).await.context("update node heartbeat")?;
    Ok(())
}

async fn assign_chunk_to_node(
    tx: &mut Transaction<'_, Postgres>,
    chunk_id: Uuid,
    replica_index: i32
) -> Result<()> {
    let node_id = env::var("NODE_ID")
        .context("NODE_ID environment variable not set")?;

    // Get node_id from node_address
    let nodeuuid = Uuid::parse_str(&node_id)?;

    // Insert into chunk_nodes table
    queries::insert_chunk_node_tx(tx, chunk_id, nodeuuid, replica_index)
        .await
        .context("Failed to assign chunk to node")?;

    Ok(())
}

pub async fn handle_node_maintenance(pool: &Pool<Postgres>) -> Result<()> {
    let now = Utc::now();
    let cutoff = now - Duration::seconds(NODE_INACTIVE_THRESHOLD_SECS);
    queries::deactivate_stale_nodes(pool, cutoff).await?;
    Ok(())
}
pub async fn cleanup_orphaned_chunks(pool: &Pool<Postgres>) -> Result<()> {
    // Start transaction for database cleanup
    let mut tx = pool.begin().await.context("begin cleanup transaction")?;

    // First cleanup database: delete chunks that don't have corresponding files
    let deleted_rows = sqlx::query!(
        r#"
        DELETE FROM file_chunks fc
        WHERE NOT EXISTS (
            SELECT 1 FROM files f
            WHERE f.file_id = fc.file_id
        )
        RETURNING file_id, chunk_index
        "#
    )
    .fetch_all(&mut *tx)
    .await?;

    // Log deleted database entries
    if !deleted_rows.is_empty() {
        info!(
            "Deleted {} orphaned chunk records from database",
            deleted_rows.len()
        );
        for row in &deleted_rows {
            info!(
                "Deleted orphaned chunk record: file_id={}, chunk_index={}",
                row.file_id, row.chunk_index
            );
        }
    }

    // Commit database cleanup
    tx.commit().await.context("commit cleanup transaction")?;

    // Now handle filesystem cleanup
    // Get all valid file IDs from the database
    let db_files = sqlx::query!(
        "SELECT file_id FROM files"
    )
    .fetch_all(pool)
    .await?;

    // Read chunks directory
    let chunks_dir = Path::new("chunks");
    if !chunks_dir.exists() {
        return Ok(());
    }

    let mut entries = fs::read_dir(chunks_dir).await?;

    // Check each file_id directory in the chunks directory
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Try to parse the directory name as UUID (file_id)
        if let Some(dir_name) = path.file_name() {
            if let Some(dir_str) = dir_name.to_str() {
                if let Ok(file_id) = Uuid::parse_str(dir_str) {
                    // If directory exists but file_id not in database, remove the whole directory
                    if !db_files.iter().any(|f| f.file_id == file_id) {
                        info!("Deleting orphaned chunk directory for file: {}", file_id);
                        match fs::remove_dir_all(&path).await {
                            Ok(_) => info!("Successfully deleted orphaned chunks for file: {}", file_id),
                            Err(e) => warn!("Failed to delete orphaned chunks for file {}: {}", file_id, e),
                        }
                    } else {
                        // Cleanup any extra chunk files that don't have corresponding database entries
                        cleanup_extra_chunk_files(pool, file_id, &path).await?;
                    }
                }
            }
        }
    }

    Ok(())
}

// Helper function to cleanup extra chunk files that don't have database entries
async fn cleanup_extra_chunk_files(pool: &Pool<Postgres>, file_id: Uuid, dir_path: &Path) -> Result<()> {
    // Get valid chunk indexes from database
    let valid_chunks = sqlx::query!(
        "SELECT chunk_index FROM file_chunks WHERE file_id = $1",
        file_id
    )
    .fetch_all(pool)
    .await?;

    let valid_indexes: Vec<i32> = valid_chunks.iter().map(|r| r.chunk_index).collect();

    // Read all chunk files in directory
    let mut entries = fs::read_dir(dir_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if let Some(filename) = path.file_name() {
            if let Some(file_str) = filename.to_str() {
                // Parse chunk index from filename (format: chunk_XXXX)
                if let Some(index_str) = file_str.strip_prefix("chunk_") {
                    if let Ok(index) = index_str.parse::<i32>() {
                        if !valid_indexes.contains(&index) {
                            info!("Deleting orphaned chunk file: {} index {}", file_id, index);
                            if let Err(e) = fs::remove_file(&path).await {
                                warn!("Failed to delete orphaned chunk file: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// Helper function to run cleanup after modifications
pub async fn run_chunk_cleanup(pool: &Pool<Postgres>) {
    if let Err(e) = cleanup_orphaned_chunks(pool).await {
        warn!("Chunk cleanup failed: {}", e);
    }
}