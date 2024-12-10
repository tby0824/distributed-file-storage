use super::Commands;
use std::fs;
use std::path::PathBuf;

use crate::proto::{AuthenticateRequest, DeleteFileRequest, DownloadRequest, GetFileMetaRequest, ListFilesRequest, UploadRequest};
use crate::proto::file_storage_client::FileStorageClient;
//use crate::proto::filestorage::{file_storage_client::FileStorageClient, AuthenticateRequest, UploadRequest, DownloadRequest, ListFilesRequest, DeleteFileRequest, GetFileMetaRequest};

fn store_token(token: &str) {
    let home = dirs::home_dir().unwrap_or(PathBuf::from("."));
    let token_path = home.join(".dfs_token");
    fs::write(token_path, token).ok();
}

fn load_token() -> Option<String> {
    let home = dirs::home_dir().unwrap_or(PathBuf::from("."));
    let token_path = home.join(".dfs_token");
    fs::read_to_string(token_path).ok()
}

pub async fn handle_cli(cmd: Commands, server_addr: &str) {
    let mut client = match FileStorageClient::connect(server_addr.to_string()).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to server: {:?}", e);
            return;
        }
    };

    match cmd {
        Commands::Login { username, password } => {
            let req = tonic::Request::new(AuthenticateRequest{ username, password });
            let resp = client.authenticate(req).await;
            match resp {
                Ok(r) => {
                    let res = r.into_inner();
                    println!("Status: {}", res.status);
                    println!("JWT: {}", res.jwt_token);
                    store_token(&res.jwt_token);
                },
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
        Commands::Upload { file_path } => {
            let token = load_token().expect("Please login first");
            let file_data = fs::read(&file_path).expect("Failed to read file");
            let file_name = file_path.split('/').last().unwrap().to_string();
            let req = tonic::Request::new(UploadRequest{
                jwt_token: token,
                file_name,
                file_data,
            });
            let resp = client.upload_file(req).await;
            match resp {
                Ok(r) => {
                    let res = r.into_inner();
                    println!("Uploaded: {}", res.file_id);
                },
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
        Commands::Download { file_id, output } => {
            let token = load_token().expect("Please login first");
            let req = tonic::Request::new(DownloadRequest{
                jwt_token: token,
                file_id,
            });
            let resp = client.download_file(req).await;
            match resp {
                Ok(r) => {
                    let res = r.into_inner();
                    if res.status == "OK" {
                        fs::write(&output, &res.file_data).expect("Failed to write file");
                        println!("Downloaded to {}", output);
                    } else {
                        eprintln!("Status: {}", res.status);
                    }
                },
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
        Commands::List {} => {
            let token = load_token().expect("Please login first");
            let req = tonic::Request::new(ListFilesRequest{ jwt_token: token });
            let resp = client.list_files(req).await;
            match resp {
                Ok(r) => {
                    let res = r.into_inner();
                    for f in res.files {
                        println!("{} - {} ({} bytes)", f.file_id, f.file_name, f.file_size);
                    }
                },
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
        },
        Commands::Delete { file_id } => {
            let token = load_token().expect("Please login first");
            let req = tonic::Request::new(DeleteFileRequest{
                jwt_token: token,
                file_id,
            });
            let resp = client.delete_file(req).await;
            match resp {
                Ok(r) => println!("Deleted: {}", r.into_inner().status),
                Err(e) => eprintln!("Error: {:?}", e),
            }
        },
        Commands::Meta { file_id } => {
            let token = load_token().expect("Please login first");
            let req = tonic::Request::new(GetFileMetaRequest{
                jwt_token: token,
                file_id,
            });
            let resp = client.get_file_meta(req).await;
            match resp {
                Ok(r) => {
                    let res = r.into_inner();
                    println!("File: {} ({} bytes)", res.file_name, res.file_size);
                },
                Err(e) => eprintln!("Error: {:?}", e),
            }
        }
    }
}
