use crate::errors::DANodeError;
use crate::node_api::config::P2PConfig;

use std::path::PathBuf;

use clap::Parser;

use config::{Config, Environment, File as ConfigFile};

#[derive(Parser, Debug)]
#[clap(version, about, author)]
pub struct CliSettings {
    /// URL of the client server to connect to, by default will be read from the CLIENT_SERVER_ENDPOINT env var or `.env` files.
    #[clap(long, short = 'C', env)]
    pub client_server_endpoint: Option<String>,

    /// JWT Server secret
    #[clap(long, short = 'J', env)]
    pub jwt_secret: Option<String>,

    /// Path to the configuration file.
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServerSettings {
    pub database: DatabaseSettings,
    #[serde(default)]
    pub client_server_endpoint: String,
    #[serde(default)]
    pub jwt_secret: secrecy::SecretString,
    #[serde(default)]
    pub p2p_config: P2PConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DatabaseSettings {
    pub path: String,
    pub name: String,
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> String {
        format!("sqlite://{}/{}.sqlite", self.path, self.name)
    }
}

pub fn get_configuration() -> Result<ServerSettings, DANodeError> {
    dotenvy::dotenv()?;
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
        database: base_settings.database,
        client_server_endpoint: cli_args
            .client_server_endpoint
            .unwrap_or(base_settings.client_server_endpoint),
        jwt_secret,
        p2p_config: base_settings.p2p_config,
    };

    Ok(settings)
}
