mod configuration;
mod errors;
mod logger;
mod user_api;

use crate::configuration::get_configuration;
use crate::logger::{get_subscriber, init_subscriber};

use tracing::error;

#[tokio::main]
async fn main() {
    init_subscriber(get_subscriber(
        "da-node".into(),
        "info".into(),
        std::io::stdout,
    ));

    let configuration = match get_configuration() {
        Ok(configuration) => configuration,
        Err(e) => {
            error!(%e, "Failed to read configuration.");
            return;
        }
    };

    println!("Hello, world!");
}
