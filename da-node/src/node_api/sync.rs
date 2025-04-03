use crate::node_api::client::PeerClient;
use crate::node_api::config::P2PConfig;
use crate::user_api::types::Blob;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use chrono::Utc;

use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePool;

use tokio::sync::RwLock;
use tokio::time::{Duration, sleep};

use uuid::Uuid;

use tracing::{error, info};

#[derive(Clone)]
pub struct SyncManager {
    config: P2PConfig,
    db_pool: SqlitePool,
    peers: HashMap<String, PeerClient>,
    blob_queue: VecDeque<(String, String)>, // (blob_id, hash)
    in_progress: HashSet<String>,
}

impl SyncManager {
    pub fn new(config: P2PConfig, db_pool: SqlitePool) -> Self {
        let peers = config
            .peers
            .iter()
            .map(|url| {
                let peer_id = Uuid::new_v4().to_string();
                (peer_id, PeerClient::new(url.clone()))
            })
            .collect();

        Self {
            config,
            db_pool,
            peers,
            blob_queue: VecDeque::new(),
            in_progress: HashSet::new(),
        }
    }

    pub fn queue_blob_fetch(&mut self, blob_id: String, hash: String) {
        if !self.in_progress.contains(&blob_id) {
            self.blob_queue.push_back((blob_id.clone(), hash));
            self.in_progress.insert(blob_id);
        }
    }

    pub async fn start_sync_loop(
        sync_manager: Arc<RwLock<Self>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        {
            let manager = sync_manager.read().await;

            // Try to connect to all peers
            for (peer_id, client) in &manager.peers {
                if let Err(e) = client.connect().await {
                    error!("Failed to connect to peer {}: {}", peer_id, e);
                } else {
                    info!("Connected to peer {}", peer_id);
                }
            }
        }

        // Start the sync loop
        loop {
            Self::process_sync_queue(sync_manager.clone()).await?;

            Self::announce_blobs(sync_manager.clone()).await?;

            // Wait for the next sync interval
            let interval;
            {
                let manager = sync_manager.read().await;
                interval = manager.config.sync_interval_secs;
            }

            sleep(Duration::from_secs(interval)).await;
        }
    }

    async fn process_sync_queue(
        sync_manager: Arc<RwLock<Self>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get a batch of blobs to process
        let mut blob_batch = Vec::new();
        {
            let mut manager = sync_manager.write().await;

            // Get up to max_concurrent_syncs blobs from the queue
            let batch_size = manager
                .config
                .max_concurrent_syncs
                .min(manager.blob_queue.len());

            for _ in 0..batch_size {
                if let Some(blob) = manager.blob_queue.pop_front() {
                    blob_batch.push(blob);
                }
            }
        }

        if blob_batch.is_empty() {
            return Ok(());
        }

        // Process blobs in parallel
        let mut join_handles = Vec::new();

        for (blob_id, hash) in blob_batch {
            let sync_manager_clone = sync_manager.clone();

            let handle = tokio::spawn(async move {
                let result = Self::fetch_and_store_blob(
                    sync_manager_clone.clone(),
                    blob_id.clone(),
                    hash.clone(),
                )
                .await;

                // Mark as no longer in progress
                {
                    let mut manager = sync_manager_clone.write().await;
                    manager.in_progress.remove(&blob_id);
                }

                result
            });

            join_handles.push(handle);
        }

        for handle in join_handles {
            if let Err(e) = handle.await? {
                error!("Error in sync task: {}", e);
            }
        }

        Ok(())
    }

    async fn fetch_and_store_blob(
        sync_manager: Arc<RwLock<Self>>,
        blob_id: String,
        expected_hash: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check if we already have this blob
        let db_pool;
        let peers;

        {
            let manager = sync_manager.read().await;
            db_pool = manager.db_pool.clone();

            // Clone peers to avoid holding the lock
            peers = manager.peers.clone();
        }

        let blob_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM blobs WHERE id = ? AND deleted_at IS NULL)",
        )
        .bind(&blob_id)
        .fetch_one(&db_pool)
        .await?;

        if blob_exists {
            // Already have this blob, nothing to do
            return Ok(());
        }

        // Try to fetch the blob from each peer
        for (peer_id, client) in peers {
            // Update sync status
            sqlx::query(
                "INSERT INTO sync_status (blob_id, peer_node_id, sync_status, last_sync_attempt)
                 VALUES (?, ?, 'pending', ?)
                 ON CONFLICT (blob_id, peer_node_id)
                 DO UPDATE SET sync_status = 'pending', last_sync_attempt = ?",
            )
            .bind(&blob_id)
            .bind(&peer_id)
            .bind(Utc::now())
            .bind(Utc::now())
            .execute(&db_pool)
            .await?;

            match client.fetch_blob(blob_id.clone(), true).await {
                Ok(Some(proto_blob)) => {
                    let Some(metadata) = &proto_blob.metadata else {
                        continue;
                    };

                    // Verify the hash
                    let mut hasher = Sha256::new();
                    hasher.update(&proto_blob.content);
                    let computed_hash = format!("{:x}", hasher.finalize());

                    if computed_hash != expected_hash && metadata.hash != expected_hash {
                        // Hash mismatch, try another peer
                        sqlx::query(
                            "UPDATE sync_status
                             SET sync_status = 'failed',
                                 last_sync_attempt = ?
                             WHERE blob_id = ? AND peer_node_id = ?",
                        )
                        .bind(Utc::now())
                        .bind(&blob_id)
                        .bind(&peer_id)
                        .execute(&db_pool)
                        .await?;

                        continue;
                    }

                    // Store the blob
                    let now = Utc::now();
                    let created_at = chrono::DateTime::parse_from_rfc3339(&metadata.created_at)
                        .unwrap_or(now.into());

                    sqlx::query(
                        "INSERT INTO blobs (
                            id, content, metadata, content_type, size, hash,
                            owner_id, created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&metadata.id)
                    .bind(&proto_blob.content)
                    .bind(&metadata.metadata)
                    .bind(&metadata.content_type)
                    .bind(metadata.size)
                    .bind(&metadata.hash)
                    .bind(&metadata.owner_id)
                    .bind(created_at)
                    .bind(now)
                    .execute(&db_pool)
                    .await?;

                    // Update sync status
                    sqlx::query(
                        "UPDATE sync_status
                         SET sync_status = 'completed',
                             last_sync_attempt = ?,
                             last_successful_sync = ?
                         WHERE blob_id = ? AND peer_node_id = ?",
                    )
                    .bind(now)
                    .bind(now)
                    .bind(&blob_id)
                    .bind(&peer_id)
                    .execute(&db_pool)
                    .await?;

                    // Successfully fetched and stored
                    return Ok(());
                }
                Ok(None) => {
                    // Blob not found on this peer
                    sqlx::query(
                        "UPDATE sync_status
                         SET sync_status = 'failed',
                             last_sync_attempt = ?
                         WHERE blob_id = ? AND peer_node_id = ?",
                    )
                    .bind(Utc::now())
                    .bind(&blob_id)
                    .bind(&peer_id)
                    .execute(&db_pool)
                    .await?;
                }
                Err(e) => {
                    error!("Error fetching blob from peer {}: {}", peer_id, e);

                    sqlx::query(
                        "UPDATE sync_status
                         SET sync_status = 'failed',
                             last_sync_attempt = ?
                         WHERE blob_id = ? AND peer_node_id = ?",
                    )
                    .bind(Utc::now())
                    .bind(&blob_id)
                    .bind(&peer_id)
                    .execute(&db_pool)
                    .await?;
                }
            }
        }

        // If we get here, we failed to fetch from any peer
        error!("Failed to fetch blob {} from any peer", blob_id);
        Ok(())
    }

    async fn announce_blobs(
        sync_manager: Arc<RwLock<Self>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db_pool;
        let peers;

        {
            let manager = sync_manager.read().await;
            db_pool = manager.db_pool.clone();
            peers = manager.peers.clone();
        }

        // Get blobs that haven't been announced to all peers
        let blobs = sqlx::query_as::<_, Blob>(
            "SELECT b.* FROM blobs b
             WHERE b.deleted_at IS NULL
             AND EXISTS (
                 SELECT 1 FROM sync_status s
                 WHERE s.blob_id = b.id
                 AND s.sync_status != 'completed'
             )
             LIMIT 100",
        )
        .fetch_all(&db_pool)
        .await?;

        if blobs.is_empty() {
            return Ok(());
        }

        // Announce each blob to peers that don't have it
        for blob in blobs {
            let metadata = crate::node_api::proto::BlobMetadata {
                id: blob.id.to_string(),
                hash: blob.hash.clone().unwrap_or_default(),
                content_type: blob.content_type.clone(),
                size: blob.size,
                owner_id: blob.owner_id.to_string(),
                metadata: blob.metadata.clone(),
                created_at: blob.created_at.to_rfc3339(),
                deleted_at: None,
            };

            // Get peers that don't have this blob
            let pending_peers = sqlx::query_scalar::<_, String>(
                "SELECT peer_node_id FROM sync_status
                 WHERE blob_id = ? AND sync_status != 'completed'",
            )
            .bind(blob.id)
            .fetch_all(&db_pool)
            .await?;

            for peer_id in pending_peers {
                if let Some(client) = peers.get(&peer_id) {
                    if let Err(e) = client.announce_blob(metadata.clone()).await {
                        eprintln!(
                            "Failed to announce blob {} to peer {}: {}",
                            blob.id, peer_id, e
                        );
                    } else {
                        // Update sync status
                        sqlx::query(
                            "UPDATE sync_status
                             SET sync_status = 'pending',
                                 last_sync_attempt = ?
                             WHERE blob_id = ? AND peer_node_id = ?",
                        )
                        .bind(Utc::now())
                        .bind(blob.id)
                        .bind(&peer_id)
                        .execute(&db_pool)
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }
}
