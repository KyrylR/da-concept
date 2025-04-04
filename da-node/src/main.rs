mod configuration;
mod errors;
mod logger;
mod node_api;
mod startup;
mod user_api;

use crate::configuration::get_configuration;
use crate::logger::{get_subscriber, init_subscriber};
use crate::startup::Application;

use std::fmt::{Debug, Display};

use tokio::task::JoinError;

use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_subscriber(get_subscriber(
        "da-node".into(),
        "info".into(),
        std::io::stdout,
    ));

    let configuration = match get_configuration() {
        Ok(configuration) => configuration,
        Err(e) => {
            error!(%e, "Failed to read configuration.");
            return Err(anyhow::anyhow!("Failed to read configuration."));
        }
    };
    let application = Application::build(configuration.clone()).await?;

    info!(
        port = application.graphql_port(),
        "Starting GraphQL server."
    );

    let application_task = tokio::spawn(application.run_until_stopped());

    tokio::select! {
        o = application_task => report_exit("API", o),
    }

    Ok(())
}

fn report_exit(task_name: &str, outcome: Result<Result<(), impl Debug + Display>, JoinError>) {
    match outcome {
        Ok(Ok(())) => {
            info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{} failed",
                task_name
            )
        }
        Err(e) => {
            error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{}' task failed to complete",
                task_name
            )
        }
    }
}
