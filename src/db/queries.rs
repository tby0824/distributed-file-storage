use sqlx::{Pool, Postgres};
use anyhow::Result;
use uuid::Uuid;
use crate::models::{User, FileMeta};

pub async fn get_user_by_username(pool: &Pool<Postgres>, username: &str) -> Result<Option<User>> {
    let res = sqlx::query_as!(
        User,
        r#"SELECT user_id, username, password, email, role, created_at, updated_at
           FROM users WHERE username = $1"#,
        username
    )
        .fetch_optional(pool)
        .await?;
    Ok(res)
}

pub async fn get_user_by_email(pool: &Pool<Postgres>, email: &str) -> Result<Option<User>> {
    let res = sqlx::query_as!(
        User,
        r#"SELECT user_id, username, password, email, role, created_at, updated_at
           FROM users WHERE email = $1"#,
        email
    )
        .fetch_optional(pool)
        .await?;
    Ok(res)
}

pub async fn insert_user(pool: &Pool<Postgres>, username: &str, password: &str, email: Option<&str>) -> Result<Uuid> {
    let rec = sqlx::query!(
        "INSERT INTO users (username, password, email) VALUES ($1, $2, $3) RETURNING user_id",
        username, password, email
    )
        .fetch_one(pool)
        .await?;
    Ok(rec.user_id)
}

pub async fn get_file_by_name(pool: &Pool<Postgres>, owner_id: Uuid, file_name: &str) -> Result<Option<FileMeta>> {
    let res = sqlx::query_as!(
        FileMeta,
        r#"SELECT file_id, owner_id, file_name, file_size, created_at, updated_at
           FROM files
           WHERE owner_id = $1 AND file_name = $2"#,
        owner_id, file_name
    )
        .fetch_optional(pool)
        .await?;
    Ok(res)
}

pub async fn get_file_by_id(pool: &Pool<Postgres>, file_id: Uuid) -> Result<Option<FileMeta>> {
    let res = sqlx::query_as!(
        FileMeta,
        r#"SELECT file_id, owner_id, file_name, file_size, created_at, updated_at
           FROM files
           WHERE file_id = $1"#,
        file_id
    )
        .fetch_optional(pool)
        .await?;
    Ok(res)
}

pub async fn insert_file_meta(pool: &Pool<Postgres>, owner_id: Uuid, file_name: &str, file_size: i64) -> Result<Uuid> {
    let rec = sqlx::query!(
        "INSERT INTO files (owner_id, file_name, file_size) VALUES ($1, $2, $3) RETURNING file_id",
        owner_id, file_name, file_size
    )
        .fetch_one(pool)
        .await?;
    Ok(rec.file_id)
}

pub async fn delete_file_by_id(pool: &Pool<Postgres>, file_id: Uuid) -> Result<u64> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "DELETE FROM chunk_nodes WHERE chunk_id IN (SELECT chunk_id FROM file_chunks WHERE file_id = $1)",
        file_id
    ).execute(&mut *tx).await?;
    sqlx::query!(
        "DELETE FROM file_chunks WHERE file_id = $1",
        file_id
    ).execute(&mut *tx).await?;

    let rows = sqlx::query!(
        "DELETE FROM files WHERE file_id = $1",
        file_id
    )
        .execute(&mut *tx)
        .await?
        .rows_affected();

    tx.commit().await?;
    Ok(rows)
}

pub async fn list_user_files(pool: &Pool<Postgres>, owner_id: Uuid) -> Result<Vec<FileMeta>> {
    let files = sqlx::query_as!(
        FileMeta,
        r#"SELECT file_id, owner_id, file_name, file_size, created_at, updated_at
           FROM files WHERE owner_id = $1
           ORDER BY created_at ASC"#,
        owner_id
    )
        .fetch_all(pool)
        .await?;
    Ok(files)
}

pub async fn rename_file(pool: &Pool<Postgres>, owner_id: Uuid, old_name: &str, new_name: &str) -> Result<u64> {
    let rows = sqlx::query!(
        "UPDATE files SET file_name = $1, updated_at = now() WHERE owner_id = $2 AND file_name = $3",
        new_name,
        owner_id,
        old_name
    )
        .execute(pool)
        .await?
        .rows_affected();
    Ok(rows)
}

pub async fn can_read_file(_pool: &Pool<Postgres>, _user_id: Uuid, _file_id: Uuid) -> Result<bool> {
    Ok(true)
}

pub async fn can_write_file(_pool: &Pool<Postgres>, _user_id: Uuid, _file_id: Uuid) -> Result<bool> {
    Ok(true)
}

pub async fn delete_user(pool: &Pool<Postgres>, user_id: Uuid) -> Result<u64> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "DELETE FROM files WHERE owner_id = $1",
        user_id
    ).execute(&mut *tx).await?;

    let rows_affected = sqlx::query!(
        "DELETE FROM users WHERE user_id = $1",
        user_id
    )
        .execute(&mut *tx)
        .await?
        .rows_affected();

    tx.commit().await?;
    Ok(rows_affected)
}

pub async fn change_user_password(pool: &Pool<Postgres>, user_id: Uuid, new_password: &str) -> Result<u64> {
    use argon2::{Argon2, PasswordHasher};
    use argon2::password_hash::SaltString;
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(new_password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?
        .to_string();

    let rows_affected = sqlx::query!(
        "UPDATE users SET password = $1, updated_at = now() WHERE user_id = $2",
        password_hash,
        user_id
    )
        .execute(pool)
        .await?
        .rows_affected();

    Ok(rows_affected)
}
