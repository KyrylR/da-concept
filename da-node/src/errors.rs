#[derive(thiserror::Error, Debug)]
pub enum DANodeError {
    #[error("failed to read env variables")]
    ReadEnvVar,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuration error: {0}")]
    Config(#[from] config::ConfigError),
}
