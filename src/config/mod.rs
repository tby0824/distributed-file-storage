use dotenv::dotenv;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub prometheus_port: u16,
    pub grpc_addr: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
        let jwt_secret = env::var("JWT_SECRET").unwrap_or("mysecret".to_string());
        let prometheus_port = env::var("PROMETHEUS_PORT").unwrap_or("9898".to_string()).parse().unwrap();
        let grpc_addr = env::var("GRPC_ADDR").unwrap_or("0.0.0.0:50051".to_string());
        Self {
            database_url,
            jwt_secret,
            prometheus_port,
            grpc_addr,
        }
    }
}
