use dotenv::dotenv;
use std::env;

// Load environment variables
pub fn load_config() {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env file");
    println!("Database URL: {}", database_url);
}