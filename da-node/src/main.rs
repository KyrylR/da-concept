mod configuration;
mod errors;
mod logger;

use crate::configuration::get_configuration;
use crate::logger::{get_subscriber, init_subscriber};

use tracing::error;

#[tokio::main]
async fn main() {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = match get_configuration() {
        Ok(configuration) => configuration,
        Err(e) => {
            error!(%e, "Failed to read configuration.");
            return;
        }
    };

    println!("Hello, world!");
}
