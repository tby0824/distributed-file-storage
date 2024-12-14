use std::env;
use dotenv::dotenv;
use anyhow::Result;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL")?;
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "jwt_secret".to_string());
        Ok(Self { database_url, jwt_secret })
    }
}
