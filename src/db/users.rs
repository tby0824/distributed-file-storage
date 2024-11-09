use sqlx::PgPool;

// username, password, email
pub async fn create_user(pool: &PgPool, username: &str, password: &str, email: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO users (username, password, email)
        VALUES ($1, $2, $3)
        "#,
        username,
        password,
        email
    )
        .execute(pool)
        .await?;

    Ok(())
}


pub async fn delete_user(pool: &PgPool, username: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        DELETE FROM users
        WHERE username = $1
        "#,
        username
    )
        .execute(pool)
        .await?;

    Ok(())
}
