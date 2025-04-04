use crate::errors::DANodeError;
use crate::node_api::client::PeerClient;
use crate::node_api::config::P2PConfig;
use crate::node_api::proto;
use crate::node_api::proto::BlobMetadata;
use crate::user_api::types::Blob;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use chrono::Utc;

use juniper::integrations::chrono::DateTime;

use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePool;

use tokio::sync::RwLock;
use tokio::time::{Duration, sleep};
use tonic::Status;
use uuid::Uuid;

use tracing::{debug, error, info};

#[derive(Clone)]
pub struct SyncManager {
    config: P2PConfig,
    db_pool: SqlitePool,
    peers: HashMap<String, PeerClient>,
    blob_queue: VecDeque<(Uuid, String)>, // (blob_id, hash)
    in_progress: HashSet<Uuid>,
}

impl SyncManager {
    pub async fn new(config: P2PConfig, db_pool: SqlitePool) -> Self {
        let mut peers: HashMap<String, PeerClient> = HashMap::new();

        for peer in config.peers.iter() {
            let peer_client = PeerClient::new(peer.clone());

            let Ok(data) = peer_client
                .handshake(&config.node_id, &config.listen_addr)
                .await
            else {
                error!("Failed to get node info from peer {}", peer);
                continue;
            };

            let Some(node_id) = data.peer_id else {
                info!("Peer rejected handshake with status {}", data.status);
                continue;
            };

            peers.insert(node_id, peer_client);
        }

        Self {
            config,
            db_pool,
            peers,
            blob_queue: VecDeque::new(),
            in_progress: HashSet::new(),
        }
    }

    #[tracing::instrument(name = "Add peer", skip(self))]
    pub async fn add_peer(&mut self, peer_id: String, peer_url: &str) {
        let peer_client = PeerClient::new(peer_url.to_string());

        self.peers.insert(peer_id, peer_client);
    }

    #[tracing::instrument(name = "Queue blob fetch", skip(self))]
    pub fn queue_blob_fetch(&mut self, blob_id: Uuid, hash: String) {
        if !self.in_progress.contains(&blob_id) {
            self.blob_queue.push_back((blob_id, hash));
            self.in_progress.insert(blob_id);
        }
    }

    pub async fn start_sync_loop(sync_manager: Arc<RwLock<Self>>) -> Result<(), DANodeError> {
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

    #[tracing::instrument(name = "Process sync queue", skip(sync_manager))]
    async fn process_sync_queue(sync_manager: Arc<RwLock<Self>>) -> Result<(), DANodeError> {
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
            info!("No blobs needed to sync.");

            return Ok(());
        }

        // Process blobs in parallel
        let mut join_handles = Vec::new();

        for (blob_id, hash) in blob_batch {
            let sync_manager_clone = sync_manager.clone();

            let handle = tokio::spawn(async move {
                let result =
                    Self::fetch_and_store_blob(sync_manager_clone.clone(), blob_id, hash.clone())
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

    #[tracing::instrument(name = "Fetch and store blob", skip(sync_manager))]
    async fn fetch_and_store_blob(
        sync_manager: Arc<RwLock<Self>>,
        blob_id: Uuid,
        expected_hash: String,
    ) -> Result<(), DANodeError> {
        // Check if we already have this blob
        let db_pool;
        let peers;
        let node_id: String;

        {
            let manager = sync_manager.read().await;
            db_pool = manager.db_pool.clone();

            // Clone peers to avoid holding the lock
            peers = manager.peers.clone();
            node_id = manager.config.node_id.clone();
        }

        if Self::does_blob_exist(&blob_id, &db_pool).await? {
            debug!(%blob_id, "Blob already exists, skipping fetch");

            for (peer_id, _) in peers {
                Self::start_blob_sync_with_peer(&blob_id, &peer_id, &db_pool).await?;
            }

            return Ok(());
        }

        // Try to fetch the blob from each peer
        for (peer_id, client) in peers {
            Self::start_blob_sync_with_peer(&blob_id, &peer_id, &db_pool).await?;

            match client.fetch_blob(&node_id, blob_id, true).await {
                Ok(Some(proto_blob)) => {
                    let Some(metadata) = &proto_blob.metadata else {
                        error!(blob_id = %blob_id, "Blob metadata is missing");

                        continue;
                    };

                    let to_store = Self::verify_received_blob(
                        metadata,
                        &proto_blob,
                        expected_hash.clone(),
                        &blob_id,
                        &peer_id,
                        &db_pool,
                    )
                    .await?;

                    // Hash mismatch, try the next peer
                    if !to_store {
                        continue;
                    }

                    let now = Utc::now();
                    Self::store_received_blob(now, metadata, &proto_blob, &db_pool).await?;
                    Self::set_blob_status_as_completed(now, &blob_id, &peer_id, &db_pool).await?;

                    return Ok(());
                }
                Ok(None) => {
                    debug!("Blob {} not found on peer {}", blob_id, peer_id);

                    Self::set_blob_status_as_failed(&blob_id, &peer_id, &db_pool).await?;
                }
                Err(e) => {
                    error!("Error fetching blob from peer {}: {}", peer_id, e);

                    Self::set_blob_status_as_failed(&blob_id, &peer_id, &db_pool).await?;
                }
            }
        }

        error!("Failed to fetch blob {} from any peer", blob_id);
        Ok(())
    }

    #[tracing::instrument(name = "Announce blobs", skip(sync_manager))]
    async fn announce_blobs(sync_manager: Arc<RwLock<Self>>) -> Result<(), DANodeError> {
        let db_pool;
        let peers;

        {
            let manager = sync_manager.read().await;
            db_pool = manager.db_pool.clone();
            peers = manager.peers.clone();
        }

        let blobs = Self::get_blobs_to_share(&db_pool).await?;
        if blobs.is_empty() {
            debug!("No blobs to share");

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
                public_key: blob.public_key.clone(),
                metadata: blob.metadata.clone(),
                created_at: blob.created_at.to_rfc3339(),
                deleted_at: None,
            };

            for peer_id in Self::get_pending_peers(&blob.id, &db_pool).await? {
                if let Some(client) = peers.get(&peer_id) {
                    match client.announce_blob(metadata.clone()).await {
                        Ok(_) => {
                            Self::set_blob_status_as_pending(&blob.id, &peer_id, &db_pool).await?
                        }
                        Err(e) => error!(
                            "Failed to announce blob {} to peer {}: {}",
                            blob.id, peer_id, e
                        ),
                    }
                }
            }
        }

        Ok(())
    }

    #[tracing::instrument(
        name = "Verify received blob",
        skip(metadata, other_blob, db_pool),
        fields(
            received_blob_id = %metadata.id,
            received_blob_hash = %metadata.hash,
            blob_id = %blob_id,
            peer_id = %peer_id,
        )
    )]
    async fn verify_received_blob(
        metadata: &BlobMetadata,
        other_blob: &proto::Blob,
        expected_hash: String,
        blob_id: &Uuid,
        peer_id: &String,
        db_pool: &SqlitePool,
    ) -> Result<bool, DANodeError> {
        let mut hasher = Sha256::new();
        hasher.update(&other_blob.content);
        let computed_hash = format!("{:x}", hasher.finalize());

        if computed_hash != expected_hash && metadata.hash != expected_hash {
            sqlx::query(
                "UPDATE sync_status
                 SET sync_status = 'failed',
                     last_sync_attempt = ?
                 WHERE blob_id = ? AND peer_node_id = ?",
            )
            .bind(Utc::now())
            .bind(blob_id)
            .bind(peer_id)
            .execute(db_pool)
            .await?;

            return Ok(false);
        }

        Ok(true)
    }

    #[tracing::instrument(
        name = "Store received blob",
        skip(db_pool, metadata, other_blob),
        fields(
            blob_id = %metadata.id,
            blob_hash = %metadata.hash,
            timestamp = %timestamp.to_rfc3339(),
        )
    )]
    async fn store_received_blob(
        timestamp: DateTime<Utc>,
        metadata: &BlobMetadata,
        other_blob: &proto::Blob,
        db_pool: &SqlitePool,
    ) -> Result<(), DANodeError> {
        let created_at =
            chrono::DateTime::parse_from_rfc3339(&metadata.created_at).unwrap_or(timestamp.into());

        sqlx::query(
            "INSERT INTO blobs (
                            id, content, metadata, content_type, size, hash,
                            owner_id, public_key, created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&metadata.id)
        .bind(&other_blob.content)
        .bind(&metadata.metadata)
        .bind(&metadata.content_type)
        .bind(metadata.size)
        .bind(&metadata.hash)
        .bind(&metadata.owner_id)
        .bind(&metadata.public_key)
        .bind(created_at)
        .bind(timestamp)
        .execute(db_pool)
        .await?;

        Ok(())
    }

    #[tracing::instrument(name = "Get blobs to share", skip(db_pool))]
    async fn get_blobs_to_share(db_pool: &SqlitePool) -> Result<Vec<Blob>, DANodeError> {
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
        .fetch_all(db_pool)
        .await?;

        Ok(blobs)
    }

    #[tracing::instrument(name = "Get peers that don't have the blob", skip(db_pool))]
    async fn get_pending_peers(
        blob_id: &Uuid,
        db_pool: &SqlitePool,
    ) -> Result<Vec<String>, DANodeError> {
        let pending_peers = sqlx::query_scalar::<_, String>(
            "SELECT peer_node_id FROM sync_status
                 WHERE blob_id = ? AND sync_status != 'completed'",
        )
        .bind(blob_id)
        .fetch_all(db_pool)
        .await?;

        Ok(pending_peers)
    }

    #[tracing::instrument(name = "Start blob sync with peer", skip(db_pool))]
    async fn start_blob_sync_with_peer(
        blob_id: &Uuid,
        peer_id: &String,
        db_pool: &SqlitePool,
    ) -> Result<(), DANodeError> {
        sqlx::query(
            "INSERT INTO sync_status (blob_id, peer_node_id, sync_status, last_sync_attempt)
                 VALUES (?, ?, 'pending', ?)
                 ON CONFLICT (blob_id, peer_node_id)
                 DO UPDATE SET sync_status = 'pending', last_sync_attempt = ?",
        )
        .bind(blob_id)
        .bind(peer_id)
        .bind(Utc::now())
        .bind(Utc::now())
        .execute(db_pool)
        .await?;

        Ok(())
    }

    #[tracing::instrument(name = "Set blob status as completed", skip(db_pool))]
    async fn set_blob_status_as_completed(
        timestamp: DateTime<Utc>,
        blob_id: &Uuid,
        peer_id: &String,
        db_pool: &SqlitePool,
    ) -> Result<(), DANodeError> {
        sqlx::query(
            "UPDATE sync_status
                         SET sync_status = 'completed',
                             last_sync_attempt = ?,
                             last_successful_sync = ?
                         WHERE blob_id = ? AND peer_node_id = ?",
        )
        .bind(timestamp)
        .bind(timestamp)
        .bind(blob_id)
        .bind(peer_id)
        .execute(db_pool)
        .await?;

        Ok(())
    }

    #[tracing::instrument(name = "Set blob status as pending", skip(db_pool))]
    async fn set_blob_status_as_pending(
        blob_id: &Uuid,
        peer_id: &String,
        db_pool: &SqlitePool,
    ) -> Result<(), DANodeError> {
        sqlx::query(
            "UPDATE sync_status
                             SET sync_status = 'pending',
                                 last_sync_attempt = ?
                             WHERE blob_id = ? AND peer_node_id = ?",
        )
        .bind(Utc::now())
        .bind(blob_id)
        .bind(peer_id)
        .execute(db_pool)
        .await?;

        Ok(())
    }

    #[tracing::instrument(name = "Set blob status as failed", skip(db_pool))]
    async fn set_blob_status_as_failed(
        blob_id: &Uuid,
        peer_id: &String,
        db_pool: &SqlitePool,
    ) -> Result<(), DANodeError> {
        sqlx::query(
            "UPDATE sync_status
                         SET sync_status = 'failed',
                             last_sync_attempt = ?
                         WHERE blob_id = ? AND peer_node_id = ?",
        )
        .bind(Utc::now())
        .bind(blob_id)
        .bind(peer_id)
        .execute(db_pool)
        .await?;

        Ok(())
    }

    #[tracing::instrument(name = "Checking if blob exists", skip(db_pool))]
    async fn does_blob_exist(blob_id: &Uuid, db_pool: &SqlitePool) -> Result<bool, Status> {
        let blob_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM blobs WHERE id = ? AND deleted_at IS NULL)",
        )
        .bind(blob_id)
        .fetch_one(db_pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(blob_exists)
    }
}
