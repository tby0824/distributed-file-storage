mod utils;
mod routers;
mod cli;
mod db;
mod handlers;
mod models;
mod services;

use clap::{Parser};
use utils::config::Config;
use env_logger;
use tokio::time::interval;
use chrono::{Utc, Duration as ChronoDuration};
use std::time::Duration;
use crate::routers::make_routes;

#[derive(Parser, Debug)]
#[command(name = "distributed-file-storage", about = "Distributed File Storage System")]
struct App {
    /// CLI
    #[arg(long, default_value_t = false)]
    cli: bool,

    #[arg(last = true)]
    cli_args: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = App::parse();

    if app.cli {
        // 如果启用了 CLI 模式，将剩余参数传递到 CLI 中解析
        cli::run_cli_with_args(app.cli_args).await?;
    } else {
        env_logger::init();
        let cfg = Config::from_env()?;
        let db_pool = db::init_pool(&cfg.database_url).await?;

        {
            let pool = db_pool.clone();
            tokio::spawn(async move {
                let mut intv = interval(Duration::from_secs(30));
                loop {
                    intv.tick().await;
                    let cutoff = Utc::now().naive_utc() - ChronoDuration::minutes(5);
                    sqlx::query("UPDATE nodes SET status='offline' WHERE last_heartbeat < $1 AND last_heartbeat IS NOT NULL")
                        .bind(cutoff)
                        .execute(&pool).await.ok();
                }
            });
        }

        let api = make_routes(db_pool.clone());
        let addr = ([0, 0, 0, 0], 8080);
        println!("Server running at http://0.0.0.0:8080");
        warp::serve(api).run(addr).await;
    }

    Ok(())
}
