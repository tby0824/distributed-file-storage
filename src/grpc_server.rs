use tonic::transport::Server;
use crate::services::file_service::FileService;
use crate::Config;
use crate::proto::file_storage_server::FileStorageServer;

pub async fn start_grpc_server(config: &Config, service: FileService) -> Result<(), Box<dyn std::error::Error>> {
    let addr = config.grpc_addr.parse()?;
    log::info!("gRPC server listening on {}", addr);
    Server::builder()
        .add_service(FileStorageServer::new(service))
        .serve(addr)
        .await?;
    Ok(())
}
