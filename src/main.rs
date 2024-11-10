use sqlx::postgres::PgPoolOptions;
use std::env;
use crate::cache::{set_key_value, get_value};
use tokio;
mod config;
mod db;
mod models;
mod cli;
mod services;
mod utils;
mod node;
mod cache;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    // load config
    config::load_config();

    // get database's URL
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env file");

    // create database's pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // test database
    match test_create_user(&pool).await {
        Ok(_) => println!("User created successfully!"),
        Err(e) => println!("Failed to create user: {}", e),
    }
    match test_query_user(&pool).await {
        Ok(_) => println!("User query executed successfully!"),
        Err(e) => println!("Failed to query user: {}", e),
    }

   // match delete_user(&pool).await {
     //   Ok(_) => println!("Test user deleted successfully!"),
       // Err(e) => println!("Failed to delete test user: {}", e),
    //}



    // Redis test - setting key-value and get value
    match set_key_value("test_key", "test_value").await {
        Ok(_) => println!("Key-value set successfully in Redis."),
        Err(err) => println!("Failed to set key-value in Redis: {}", err),
    }
    match get_value("test_key").await {
        Ok(value) => println!("Value retrieved from Redis: {}", value),
        Err(err) => println!("Failed to get value from Redis: {}", err),
    }


    Ok(())
}





// testing database
async fn test_create_user(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    db::users::create_user(pool, "test2", "password", "11@ee.com").await
}

async fn test_query_user(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT username FROM users
        LIMIT 1
        "#
    )
        .fetch_all(pool)
        .await?;

    if rows.is_empty() {
        println!("No users found in the database.");
    } else {
        println!("Successfully fetched user: {}", rows[0].username);
    }

    Ok(())
}

async fn delete_user(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    db::users::delete_user(&pool, "test1").await
}