#[derive(thiserror::Error, Debug)]
pub enum DANodeError {
    #[error("failed to read env variables: {0}")]
    ReadEnvVar(#[from] dotenvy::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuration error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("gRPC communication error: {0}")]
    GRPCClient(#[from] tonic::transport::Error),
    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),
    #[error("gRPC client not connected. ID: {0}")]
    ClientNotConnected(String),
    #[error("sqlx error: {0}")]
    DatabaseConnection(#[from] sqlx::Error),
    #[error("failed to parse socket address:{0}")]
    AddrParse(#[from] std::net::AddrParseError),
    #[error("subtask error: {0}")]
    Subtask(#[from] tokio::task::JoinError),
    #[error("received invalid response from peer: {0}")]
    InvalidResponse(String),
}
