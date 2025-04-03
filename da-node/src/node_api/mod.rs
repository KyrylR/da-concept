pub mod client;
pub mod config;
pub mod server;
pub mod sync;

pub mod proto {
    tonic::include_proto!("p2p.sync");
}

use crate::errors::DANodeError;
use crate::node_api::config::P2PConfig;
use crate::node_api::sync::SyncManager;

use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::RwLock;

use tracing::error;

/// Initialize the P2P system with the given configuration
pub async fn init_p2p(
    config: P2PConfig,
    db_pool: SqlitePool,
) -> Result<Arc<RwLock<SyncManager>>, Box<dyn std::error::Error + Send + Sync>> {
    let sync_manager = Arc::new(RwLock::new(SyncManager::new(
        config.clone(),
        db_pool.clone(),
    )));

    let server_config = config.clone();
    let server_db_pool = db_pool.clone();
    let server_sync_manager = sync_manager.clone();

    tokio::spawn(async move {
        if let Err(e) =
            server::create_grpc_server(server_config, server_db_pool, server_sync_manager).await
        {
            error!(%e, "P2P server error");
        }
    });

    let sync_loop_manager = sync_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = SyncManager::start_sync_loop(sync_loop_manager).await {
            error!(%e, "P2P sync loop error");
        }
    });

    Ok(sync_manager)
}

/// Enqueue a blob for syncing to all peers
pub async fn sync_blob(
    sync_manager: &Arc<RwLock<SyncManager>>,
    blob_id: String,
    hash: String,
) -> Result<(), DANodeError> {
    let mut manager = sync_manager.write().await;
    manager.queue_blob_fetch(blob_id, hash);
    Ok(())
}
