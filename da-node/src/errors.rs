#[derive(thiserror::Error, Debug)]
pub enum DANodeError {
    #[error("failed to read env variables")]
    ReadEnvVar,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuration error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("gRPC communication error: {0}")]
    GRPCClient(#[from] tonic::transport::Error),
    #[error("gRPC error: {0}")]
    GRPC(#[from] tonic::Status),
    #[error("gRPC client not connected. ID: {0}")]
    ClientNotConnected(String),
}
