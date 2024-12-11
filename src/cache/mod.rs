use redis::AsyncCommands;
use redis::Client;
use std::env;

// connection with Redis using MultiplexedConnection
pub async fn get_redis_connection() -> Result<redis::aio::MultiplexedConnection, redis::RedisError> {
    let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set in .env file");
    let client = Client::open(redis_url)?;
    let connection = client.get_multiplexed_async_connection().await?;
    Ok(connection)
}

// set a key-value pair in Redis
pub async fn set_key_value(key: &str, value: &str) -> Result<(), redis::RedisError> {
    let mut con: redis::aio::MultiplexedConnection = get_redis_connection().await?;
    let _: () = con.set(key, value).await?;
    Ok(())
}

// get a value from Redis
pub async fn get_value(key: &str) -> Result<String, redis::RedisError> {
    let mut con: redis::aio::MultiplexedConnection = get_redis_connection().await?;
    let value: String = con.get(key).await?;
    Ok(value)
}
