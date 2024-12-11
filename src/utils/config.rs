use anyhow::Result;

pub struct Config {
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();
        let database_url = std::env::var("DATABASE_URL")?;
        Ok(Config {
            database_url
        })
    }
}
