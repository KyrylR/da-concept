pub mod client;
pub mod config;
pub mod server;
pub mod sync;

pub mod proto {
    tonic::include_proto!("p2p.sync");
}

use crate::errors::DANodeError;
use crate::node_api::sync::SyncManager;

use std::sync::Arc;

use tokio::sync::RwLock;

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
