use clap::{Parser, Subcommand};
use anyhow::Result;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use uuid::Uuid;

/// CLI
#[derive(Parser, Debug)]
#[command(name = "dfs-cli", about = "Distributed File Storage CLI")]
struct CLI {
    ///http://localhost:8080
    #[arg(short, long, default_value = "http://localhost:8080")]
    server: String,

    /// JWT Token
    #[arg(short, long)]
    token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Register {
        #[arg(long)]
        username: String,

        #[arg(long)]
        password: String,

        #[arg(long)]
        email: Option<String>,
    },
    Upload {
        #[arg(long)]
        owner_id: Uuid,
        #[arg(long)]
        file_name: String,
        #[arg(long)]
        file_path: String,
        #[arg(long, default_value = "1024")]
        chunk_size: usize,
    },
    Download {
        #[arg(long)]
        file_id: Uuid,
        #[arg(long)]
        output: String,
    },
    RegisterNode {
        #[arg(long)]
        node_address: String,
    },
    ListNodes,
}

pub async fn run_cli_with_args(args: Vec<String>) -> Result<()> {
    let mut cli_args = vec!["dfs-cli".to_string()];
    cli_args.extend(args);

    let cli = CLI::parse_from(cli_args);

    let client = reqwest::Client::new();

    match cli.command {
        Commands::Register {
            username,
            password,
            email,
        } => {
            let resp = client
                .post(format!("{}/register", cli.server))
                .json(&json!({
                    "username": username,
                    "password": password,
                    "email": email
                }))
                .send()
                .await?;

            let text = resp.text().await?;
            println!("{}", text);
        }

        Commands::Upload {
            owner_id,
            file_name,
            file_path,
            chunk_size,
        } => {
            let token = cli.token.ok_or(anyhow::anyhow!("Token required for upload"))?;
            let resp = client
                .post(format!("{}/upload", cli.server))
                .bearer_auth(token)
                .json(&json!({
                    "owner_id": owner_id,
                    "file_name": file_name,
                    "file_path": file_path,
                    "chunk_size": chunk_size
                }))
                .send()
                .await?;
            let text = resp.text().await?;
            println!("{}", text);
        }

        Commands::Download { file_id, output } => {
            let token = cli.token.ok_or(anyhow::anyhow!("Token required for download"))?;
            let resp = client
                .get(format!("{}/download/{}", cli.server, file_id))
                .bearer_auth(token)
                .send()
                .await?;

            if resp.status().is_success() {
                let bytes = resp.bytes().await?;
                let mut file = File::create(output)?;
                file.write_all(&bytes)?;
                println!("File downloaded successfully.");
            } else {
                println!("Failed to download file. Status: {}", resp.status());
                let text = resp.text().await?;
                println!("Response: {}", text);
            }
        }

        Commands::RegisterNode { node_address } => {
            let resp = client
                .post(format!("{}/register_node", cli.server))
                .json(&json!({
                    "node_address": node_address
                }))
                .send()
                .await?;

            let text = resp.text().await?;
            println!("{}", text);
        }

        Commands::ListNodes => {
            let resp = client
                .get(format!("{}/nodes", cli.server))
                .send()
                .await?;

            let text = resp.text().await?;
            println!("{}", text);
        }
    }

    Ok(())
}
