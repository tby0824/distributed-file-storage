use sqlx::{Pool, Postgres};
use crate::models::user::User;
use uuid::Uuid;

pub async fn create_user(pool: &Pool<Postgres>, username: &str, password_hash: &str, email: Option<&str>) -> Result<User, sqlx::Error> {
    let record = sqlx::query_as!(User,
        r#"
        INSERT INTO users (username, password, email)
        VALUES ($1, $2, $3)
        RETURNING user_id, username, password, email, role, created_at, updated_at
        "#,
        username, password_hash, email
    )
        .fetch_one(pool)
        .await?;
    Ok(record)
}

pub async fn get_user_by_username(pool: &Pool<Postgres>, username: &str) -> Result<User, sqlx::Error> {
    let record = sqlx::query_as!(User,
        r#"
        SELECT user_id, username, password, email, role, created_at, updated_at
        FROM users
        WHERE username = $1
        "#,
        username
    )
        .fetch_one(pool)
        .await?;
    Ok(record)
}
