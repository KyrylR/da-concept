mod configuration;
mod errors;
mod logger;
mod node_api;
mod startup;
mod user_api;

use crate::configuration::get_configuration;
use crate::logger::{get_subscriber, init_subscriber};
use crate::node_api::sync::SyncManager;
use crate::startup::{Application, P2P, get_connection_pool};

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
    let connection_pool = get_connection_pool(&configuration.database_url)?;

    let p2p = P2P::try_from(configuration.p2p_config.clone(), connection_pool.clone()).await?;
    let sync_manager = p2p.sync_manager.clone();
    let application = Application::build(
        configuration.clone(),
        sync_manager.clone(),
        connection_pool.clone(),
    )
    .await?;

    let application_task = tokio::spawn(application.run_until_stopped());
    let p2p_task = tokio::spawn(p2p.run_until_stopped());
    let sync_manager_task = tokio::spawn(SyncManager::start_sync_loop(sync_manager.clone()));

    tokio::select! {
        o = application_task => report_exit("API", o),
        o = p2p_task => report_exit("P2P", o),
        o = sync_manager_task => report_exit("SyncManager", o),
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
