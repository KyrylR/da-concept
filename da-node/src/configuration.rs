use crate::errors::DANodeError;

use std::path::PathBuf;

use clap::Parser;
use config::{Config, Environment, File as ConfigFile};

#[derive(Parser, Debug)]
#[clap(version, about, author)]
pub struct CliSettings {
    /// Location of the DB, by default will be read from the DATABASE_URL env var or `.env` files.
    #[clap(long, short = 'D', env)]
    pub database_url: Option<String>,

    /// URL of the server to connect to, by default will be read from the SERVER_URL env var or `.env` files.
    #[clap(long, short = 'S', env)]
    pub server_endpoint: Option<String>,

    /// JWT Server secret
    #[clap(long, short = 'J', env)]
    pub jwt_secret: Option<String>,

    /// Path to the configuration file.
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServerSettings {
    #[serde(default)]
    pub database_url: String,
    #[serde(default)]
    pub server_endpoint: String,
    pub jwt_secret: secrecy::SecretString,

    pub node_id: String,
    pub p2p_listen_addr: String,
    pub p2p_peers: Vec<String>,
    pub p2p_sync_interval_secs: u64,
}

pub fn get_configuration() -> Result<ServerSettings, DANodeError> {
    let Ok(_) = dotenvy::dotenv() else {
        return Err(DANodeError::ReadEnvVar);
    };
    let cli_args = CliSettings::parse();

    let config_path = std::env::var_os("CONF_FILE")
        .map(PathBuf::from)
        .or(cli_args.config);

    let is_config_exists = config_path.is_some();
    let config_builder = Config::builder()
        .add_source(ConfigFile::from(config_path.unwrap_or_default()).required(is_config_exists))
        .add_source(
            Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        );

    let base_settings: ServerSettings = config_builder.build()?.try_deserialize()?;

    let jwt_secret: secrecy::SecretString = if let Some(secret) = cli_args.jwt_secret {
        secret.into()
    } else {
        base_settings.jwt_secret
    };

    let settings = ServerSettings {
        database_url: cli_args.database_url.unwrap_or(base_settings.database_url),
        server_endpoint: cli_args
            .server_endpoint
            .unwrap_or(base_settings.server_endpoint),
        jwt_secret,
        node_id: base_settings.node_id,
        p2p_listen_addr: base_settings.p2p_listen_addr,
        p2p_peers: base_settings.p2p_peers,
        p2p_sync_interval_secs: base_settings.p2p_sync_interval_secs,
    };

    Ok(settings)
}
