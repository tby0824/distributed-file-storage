// src/main.rs
mod config;
mod db;
mod models;
mod utils;
mod cli;
mod services;
mod node;

mod grpc_server;
mod distribution;
mod proto;

use clap::Parser;
use cli::{Cli, commands::handle_cli};
use config::Config;
use tokio::task;
use warp::Filter;
use prometheus::{Encoder, TextEncoder, register_int_counter, register_histogram, IntCounter, Histogram};
use log::info;
use env_logger::Env;
use sqlx::Pool;
use sqlx::Postgres;
use services::file_service::FileService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // 加载配置
    let config = Config::from_env();

    // 初始化数据库连接池
    let pool = db::init_db(&config.database_url).await?;

    // 启动节点发现与通信
    let communication = node::node_discovery::start_node_discovery().await?;

    // 注册 Prometheus 指标
    let uploaded_files_counter = register_int_counter!("uploaded_files_total", "Total uploaded files")?;
    let downloaded_files_counter = register_int_counter!("downloaded_files_total", "Total downloaded files")?;
    let request_histogram = register_histogram!("request_duration_seconds", "The request latencies in seconds")?;

    // 创建 FileService 实例
    let service = FileService {
        pool: pool.clone(),
        jwt_secret: config.jwt_secret.clone(),
        uploaded_files_counter,
        downloaded_files_counter,
        request_histogram,
        communication: communication.clone(),
    };

    // 启动 gRPC 服务器
    let grpc_handle = {
        let config = config.clone();
        let service = service.clone();
        task::spawn(async move {
            if let Err(e) = grpc_server::start_grpc_server(&config, service).await {
                eprintln!("gRPC server error: {:?}", e);
            }
        })
    };

    // Prometheus metrics endpoint
    let metrics_route = warp::path("metrics").map(move || {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        warp::reply::with_header(buffer, "Content-Type", encoder.format_type())
    });

    let warp_handle = {
        let port = config.prometheus_port;
        task::spawn(async move {
            info!("Prometheus metrics at 0.0.0.0:{}", port);
            warp::serve(metrics_route).run(([0, 0, 0, 0], port)).await;
        })
    };

    // 处理 CLI 命令
    let cli = Cli::parse();
    handle_cli(cli.command, &cli.server_addr).await;

    // 等待 gRPC 和 Prometheus 任务完成
    grpc_handle.await?;
    warp_handle.await?;
    Ok(())
}
