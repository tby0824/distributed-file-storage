use libp2p::{identity, Multiaddr};
use libp2p::{
    core::upgrade,
    futures::StreamExt,
    mplex,
    noise::{Keypair, NoiseConfig, X25519Spec},
    swarm::SwarmEvent,
    tcp::TokioTcpConfig,
    Transport,
};
use libp2p::PeerId;
use std::error::Error;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use std::fs;
use std::path::Path;
use log::{info, error};
use env_logger;
use std::env;

mod db;
mod handlers;
mod models;
mod utils;
mod network;

use utils::config::Config;
use handlers::login_user;
use network::{
    UploadRequest, RegisterRequest, BatchUploadRequest, BatchDeleteRequest, ListFilesRequest,
    RenameFileRequest, FileRequest, FileResponse, DownloadRequest, DeleteFileRequest, FileStorageProtocol
};
use serde::{Serialize, Deserialize};
use rustyline::DefaultEditor;

#[derive(Serialize, Deserialize)]
struct Session {
    user_id: Uuid,
    username: String,
    email: Option<String>,
    role: String,
    created_at: String,
    jwt: String,
}

impl Session {
    fn load() -> Option<Self> {
        if Path::new(".session.json").exists() {
            let data = fs::read_to_string(".session.json").ok()?;
            serde_json::from_str(&data).ok()
        } else {
            None
        }
    }

    fn save(&self) {
        let data = serde_json::to_string_pretty(self).expect("Serialize session");
        fs::write(".session.json", data).expect("Write session");
    }

    fn clear() {
        let _ = fs::remove_file(".session.json");
    }

    fn logged_in() -> bool {
        Path::new(".session.json").exists()
    }
}

fn check_login() -> Result<Session, Box<dyn Error>> {
    if let Some(s) = Session::load() {
        Ok(s)
    } else {
        Err("Please login first.".into())
    }
}

#[derive(Debug)]
enum SwarmCommand {
    LocalRequest(FileRequest, oneshot::Sender<FileResponse>),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let cfg = Config::from_env()?;
    info!("Starting with DB: {}", cfg.database_url);

    let db_pool = db::init_pool(&cfg.database_url).await?;
    let _ = sqlx::query!("SELECT 1 as one").fetch_one(&db_pool).await?;
    info!("Database OK.");

    tokio::fs::create_dir_all("chunks").await?;

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    info!("Local peer id: {:?}", local_peer_id);

    let port = if let Ok(addr) = env::var("NODE_ADDRESS") {
        if let Some(port_str) = addr.split("/").nth(4) {
            match port_str.parse::<u16>() {
                Ok(p) => {
                    info!("Using port {} from NODE_ADDRESS", p);
                    p
                },
                Err(e) => {
                    error!("Failed to parse port number from NODE_ADDRESS: {}", e);
                    return Err("Invalid port number in NODE_ADDRESS".into());
                }
            }
        } else {
            error!("Invalid NODE_ADDRESS format: missing port number component");
            return Err("NODE_ADDRESS must contain a port number (expected format: /ip4/.../.../port)".into());
        }
    } else {
        error!("NODE_ADDRESS environment variable not set");
        return Err("NODE_ADDRESS environment variable must be set".into());
    };
    let noise_keys = Keypair::<X25519Spec>::new().into_authentic(&local_key)?;
    let transport = TokioTcpConfig::new()
        .nodelay(true)
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let protocol = FileStorageProtocol::new(db_pool.clone(), cfg.jwt_secret.clone());

    // 定期维护节点状态（示意，每60秒检查一次）
    let maintenance_pool = db_pool.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = handlers::handle_node_maintenance(&maintenance_pool).await {
                error!("Node maintenance error: {}", e);
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });

    let mut swarm = libp2p::Swarm::new(transport, protocol, local_peer_id);


    swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{}", port).parse::<Multiaddr>()?)?;


    let (tx, mut rx) = mpsc::channel::<SwarmCommand>(32);

    let mut last_connected_peer: Option<PeerId> = None;
    

    // 后台任务负责处理swarm事件
    tokio::spawn(async move {
        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        
                        Some(SwarmCommand::LocalRequest(req, reply)) => {
                            let resp = swarm.behaviour().handle_local_request(req).await;
                            let _ = reply.send(resp);
                        }
                        
                        None => break,
                    }
                }
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!("Listening on {:?}", address);
                            println!("Listening on {:?}", address);
                            if address.to_string().starts_with("/ip4/127.") {
                                env::set_var("NODE_IP", address.to_string());
                            }
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            info!("Connected to {:?}", peer_id);
                            last_connected_peer = Some(peer_id);
                        }
                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            info!("Disconnected from {:?}", peer_id);
                            if last_connected_peer == Some(peer_id) {
                                last_connected_peer = None;
                            }
                        }
                        SwarmEvent::Behaviour(network::FileStorageProtocolEvent::ResponseReceived { peer, response }) => {
                            println!("Got response from {:?}: {:?}", peer, response);
                        }
                        _ => {}
                    }
                }
            }
        }
    });
    
    
    let mut rl = DefaultEditor::new()?;
    println!("Welcome to the distributed-file-storage interactive CLI!");
    println!("Type 'help' for available commands, 'exit' to quit.");

    let cache = utils::cache::Cache::new("redis://127.0.0.1/");

    async fn request_local(tx: &mpsc::Sender<SwarmCommand>, req: FileRequest) -> FileResponse {
        let (reply_tx, reply_rx) = oneshot::channel();
        if tx.send(SwarmCommand::LocalRequest(req, reply_tx)).await.is_err() {
            return FileResponse::Error("Failed to send request".into());
        }
        reply_rx.await.unwrap_or_else(|_| FileResponse::Error("Reply canceled".into()))
    }
    let mut attempts = 0;
    let max_attempts = 30; // Wait up to 30 seconds

    while env::var("NODE_IP").is_err() {
        if attempts >= max_attempts {
            error!("Timeout waiting for node address");
            return Err("Timeout waiting for node address".into());
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        attempts += 1;
    }
    
    // Read the environment variable
    let node_address = env::var("NODE_IP")?;
    println!("Got node address: {}", node_address);    
    let req = FileRequest::RegisterNode(node_address.clone(), None);
    let _resp = request_local(&tx, req).await;
    let heartbeat_tx = tx.clone();
    let node_record = sqlx::query!(
        "SELECT node_id FROM nodes WHERE node_address = $1",
        node_address
    )
    .fetch_optional(&db_pool)
    .await?;
    let node_id = match node_record {
        Some(record) => record.node_id,
        None => {
            error!("No node found with address: {}", &node_address);
            return Err("Node not found in database".into());
        }
    };
    env::set_var("NODE_ID", node_id.to_string());

    tokio::spawn(async move {
        let heartbeat_interval = std::time::Duration::from_secs(30);
        loop {
            let req = FileRequest::NodeHeartbeat(node_id, None);
            let resp = request_local(&heartbeat_tx, req).await;
            
            match resp {
                FileResponse::Error(e) => {
                    error!("Heartbeat failed for node {}: {}", node_id, e);
                },
                _ => {
                    info!("Heartbeat successful for node {}", node_id);
                }
            }
    
            tokio::time::sleep(heartbeat_interval).await;
        }
    });


    loop {

    
    // Check if it's time for a heartbeat
    // if last_heartbeat.elapsed() >= heartbeat_interval {
    //     if let Some(node_id) = current_node_id {  // Add this variable at the top of your program
    //         let req = FileRequest::NodeHeartbeat(node_id, None);
    //         if let Err(e) = tx.send(SwarmCommand::Request(req)).await {
    //             println!("Failed to send heartbeat: {}", e);
    //         }
    //         last_heartbeat = std::time::Instant::now();
    //     }
    // }
        let line = rl.readline("dfs> ");
        match line {
            Ok(cmdline) => {
                let _ = rl.add_history_entry(cmdline.as_str());
                let args: Vec<_> = cmdline.split_whitespace().collect();
                if args.is_empty() {
                    continue;
                }

                // 从session中获取jwt（如果已登录）
                let jwt = Session::load().map(|s| s.jwt);
                

                match args[0] {
                    "exit" => break,
                    "help" => {
                        println!("Commands:");
                        println!("  register <username> <password> [email]");
                        println!("  login <username> <password>");
                        println!("  whoami");
                        println!("  upload <file_path>");
                        println!("  download <file_id>");
                        println!("  delete <file_id> [--confirm]");
                        println!("  batch-upload <file1> <file2> ... [--force]");
                        println!("  batch-delete <file1> <file2> ... [--confirm]");
                        println!("  list-files");
                        println!("  rename-file <old_name> <new_name>");
                        println!("  list-nodes");
                        println!("  logout");
                        println!("  exit");
                    }

                    
                    "register" => {
                        if args.len() < 3 {
                            println!("Usage: register <username> <password> [email]");
                            continue;
                        }
                        let username = args[1].to_string();
                        let password = args[2].to_string();
                        let email = if args.len() > 3 { Some(args[3].to_string()) } else { None };
                        let req = FileRequest::Register(RegisterRequest { username, password, email }, None);
                        let resp = request_local(&tx, req).await;
                        println!("Response: {:?}", resp);
                    }
                    "login" => {
                        if args.len() < 3 {
                            println!("Usage: login <username> <password>");
                            continue;
                        }
                        let username = args[1];
                        let password = args[2];
                        match login_user(&db_pool, username, password).await {
                            Ok(Some(user)) => {
                                let token = match handlers::generate_jwt(&cfg.jwt_secret, &user) {
                                    Ok(t) => t,
                                    Err(e) => { println!("JWT error: {}", e); continue; }
                                };
                                let session = Session {
                                    user_id: user.user_id,
                                    username: user.username.clone(),
                                    email: user.email.clone(),
                                    role: user.role.clone(),
                                    created_at: user.created_at.to_string(),
                                    jwt: token,
                                };
                                session.save();
                                println!("Login successful!");
                            }
                            Ok(None) => println!("Login failed."),
                            Err(e) => println!("Error: {}", e),
                        }
                    }
                    "whoami" => {
                        if let Some(s) = Session::load() {
                            println!("Logged in as: {}", s.username);
                            println!("Email: {:?}", s.email);
                            println!("Role: {}", s.role);
                            println!("Created At: {}", s.created_at);
                            println!("JWT: {}", s.jwt);
                        } else {
                            println!("Not logged in.");
                        }
                    }
                    "upload" => {
                        println!("{}", args[0]);
                        let session = match check_login() {
                            Ok(s) => s,
                            Err(e) => { println!("{}", e); continue; }
                        };
                        if args.len() < 2 {
                            println!("Usage: upload <file_path>");
                            continue;
                        }
                        let file_path = args[1];
                        let data = match std::fs::read(file_path) {
                            Ok(d) => d,
                            Err(e) => { println!("Failed to read '{}': {}", file_path, e); continue; }
                        };
                        let req = FileRequest::Upload(UploadRequest {
                            owner_id: session.user_id,
                            file_name: file_path.to_string(),
                            file_data: data
                        }, jwt.clone());
                        let resp = request_local(&tx, req).await;
                        println!("Response: {:?}", resp);
                        cache.invalidate_file_list(session.user_id).await?;
                    }
                    "download" => {
                        let _session = match check_login() {
                            Ok(s) => s,
                            Err(e) => { println!("{}", e); continue; }
                        };
                        
                        if args.len() < 2 {
                            println!("Usage: download <file_id>");
                            continue;
                        }
                        
                        // Ensure local_files directory exists
                        if !std::path::Path::new("local_files").exists() {
                            match std::fs::create_dir("local_files") {
                                Ok(_) => println!("Created local_files directory"),
                                Err(e) => {
                                    println!("Failed to create local_files directory: {}", e);
                                    continue;
                                }
                            }
                        }
                        
                        let file_id = match Uuid::parse_str(args[1]) {
                            Ok(id) => id,
                            Err(_) => { println!("Invalid file_id UUID"); continue; }
                        };
                        
                        let req = FileRequest::Download(DownloadRequest{ file_id }, jwt.clone());
                        let resp = request_local(&tx, req).await;
                        
                        match resp {
                            FileResponse::Download(d) => {
                                let file_path = std::path::Path::new("local_files").join(&d.file_name);
                                
                                match fs::write(&file_path, &d.file_data) {
                                    Ok(_) => println!("Downloaded file: {} ({} bytes)", d.file_name, d.file_data.len()),
                                    Err(e) => println!("Failed to write file: {}", e)
                                }
                            }
                            FileResponse::Error(e) => println!("Error: {}", e),
                            _ => println!("Unexpected response")
                        }
                    }
                    "delete" => {
                        let session = match check_login() {
                            Ok(s) => s,
                            Err(e) => { println!("{}", e); continue; }
                        };
                        if args.len() < 2 {
                            println!("Usage: delete <file_id> [--confirm]");
                            continue;
                        }
                        let file_id = match Uuid::parse_str(args[1]) {
                            Ok(id) => id,
                            Err(_) => { println!("Invalid file_id UUID"); continue; }
                        };
                        let confirm = args.contains(&"--confirm");
                        if !confirm {
                            println!("Add --confirm to actually delete the file.");
                            continue;
                        }
                        let req = FileRequest::DeleteFile(DeleteFileRequest {
                            file_id,
                            user_id: session.user_id
                        }, jwt.clone());
                        let resp = request_local(&tx, req).await;
                        println!("Response: {:?}", resp);
                        cache.invalidate_file_list(session.user_id).await?;
                    }
                    "batch-upload" => {
                        let session = match check_login() {
                            Ok(s) => s,
                            Err(e) => { println!("{}", e); continue; }
                        };
                        if args.len() < 2 {
                            println!("Usage: batch-upload <files...> [--force]");
                            continue;
                        }
                        let force = args.contains(&"--force");
                        let file_paths: Vec<_> = args[1..].iter().filter(|x| **x != "--force").map(|x| x.to_string()).collect();
                        let req = FileRequest::BatchUpload(BatchUploadRequest {
                            owner_id: session.user_id,
                            file_paths,
                            force,
                        }, jwt.clone());
                        let resp = request_local(&tx, req).await;
                        println!("Response: {:?}", resp);
                        cache.invalidate_file_list(session.user_id).await?;
                    }
                    "batch-delete" => {
                        let session = match check_login() {
                            Ok(s) => s,
                            Err(e) => { println!("{}", e); continue; }
                        };
                        let dry_run = args.contains(&"--dry-run");
                        let confirm = args.contains(&"--confirm");
                        if !dry_run && !confirm {
                            println!("Add --confirm to actually delete files or --dry-run to test.");
                            continue;
                        }
                        let file_names: Vec<_> = args[1..].iter().filter(|&x| *x != "--dry-run" && *x != "--confirm").map(|x| x.to_string()).collect();
                        let req = FileRequest::BatchDelete(BatchDeleteRequest {
                            user_id: session.user_id,
                            file_names,
                        }, jwt.clone());
                        let resp = request_local(&tx, req).await;
                        println!("Response: {:?}", resp);
                        if confirm {
                            cache.invalidate_file_list(session.user_id).await?;
                        }
                    }
                    "list-files" => {
                        let session = match check_login() {
                            Ok(s) => s,
                            Err(e) => { println!("{}", e); continue; }
                        };
                        // 尝试cache
                        if let Some(files) = cache.get_file_list(session.user_id).await? {
                            println!("(From cache) Your files:");
                            for f in files {
                                println!("- {} ({} bytes, file_id={})", f.file_name, f.file_size, f.file_id);
                            }
                        } else {
                            let req = FileRequest::ListFiles(ListFilesRequest{owner_id:session.user_id}, jwt.clone());
                            let resp = request_local(&tx, req).await;
                            match resp {
                                FileResponse::ListFiles(r) => {
                                    if r.files.is_empty() {
                                        println!("You have no files.");
                                    } else {
                                        println!("Your files:");
                                        for file in &r.files {
                                            println!("- {} ({} bytes) [ID: {}]", file.file_name, file.file_size, file.file_id);
                                        }
                                    }
                                },
                                FileResponse::Error(e) => println!("Error: {}", e),
                                _ => println!("Unexpected response")
                            }
                        }
                    }
                    "rename-file" => {
                        let session = match check_login() {
                            Ok(s) => s,
                            Err(e) => { println!("{}", e); continue; }
                        };
                        if args.len() < 3 {
                            println!("Usage: rename-file <old_name> <new_name>");
                            continue;
                        }
                        let old_name = args[1].to_string();
                        let new_name = args[2].to_string();
                        let req = FileRequest::RenameFile(RenameFileRequest {
                            user_id: session.user_id,
                            old_name,
                            new_name
                        }, jwt.clone());
                        let resp = request_local(&tx, req).await;
                        println!("Response: {:?}", resp);
                        cache.invalidate_file_list(session.user_id).await?;
                    }
                    "list-nodes" => {
                        let req = FileRequest::ListNodes(None);
                        let resp = request_local(&tx, req).await;
                        println!("Response: {:?}", resp);
                    }
                    
                    // "node-heartbeat" => {
                    //     if args.len() < 2 {
                    //         println!("Usage: node-heartbeat <node_id>");
                    //         continue;
                    //     }
                    //     let node_id = match Uuid::parse_str(args[1]) {
                    //         Ok(n) => n,
                    //         Err(_) => {println!("Invalid node_id"); continue;}
                    //     };
                    //     let req = FileRequest::NodeHeartbeat(node_id, None);
                    //     let resp = request_local(&tx, req).await;
                    //     println!("Response: {:?}", resp);
                    // }
                    "logout" => {
                        if Session::logged_in() {
                            Session::clear();
                            println!("Logged out.");
                        } else {
                            println!("Not logged in.");
                        }
                    }
                    other => {
                        println!("Unknown command '{}'. Type 'help' for commands.", other);
                    }
                }
            }
            Err(e) => {
                println!("Error reading line: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}
