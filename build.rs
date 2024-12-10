fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .compile_protos(&["src/proto/file_storage.proto"], &["src/proto"])?;
    Ok(())
}
