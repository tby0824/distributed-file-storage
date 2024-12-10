use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dfs-cli")]
#[command(about = "A CLI for distributed file storage system", long_about = None)]
pub struct Cli {
    #[arg(short, long, default_value="http://127.0.0.1:50051")]
    pub server_addr: String,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Login {
        #[arg(short, long)]
        username: String,
        #[arg(short, long)]
        password: String,
    },
    Upload {
        #[arg(short, long)]
        file_path: String,
    },
    Download {
        #[arg(short, long)]
        file_id: String,
        #[arg(short, long, default_value="output_file")]
        output: String,
    },
    List {},
    Delete {
        #[arg(short, long)]
        file_id: String,
    },
    Meta {
        #[arg(short, long)]
        file_id: String,
    }
}

pub mod commands;
