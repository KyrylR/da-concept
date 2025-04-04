use crate::node_api::config::P2PConfig;
use crate::node_api::proto::sync_service_server::{SyncService, SyncServiceServer};
use crate::node_api::proto::{
    AnnounceBlobRequest, AnnounceBlobResponse, BlobMetadata, DeleteBlobRequest, DeleteBlobResponse,
    FetchBlobRequest, FetchBlobResponse, HandshakeRequest, HandshakeResponse, HandshakeStatus,
    NodeInfoResponse, PeerSyncStatus, SyncRequest, SyncResponse, SyncStatus, SyncStatusRequest,
    SyncStatusResponse,
};
use crate::node_api::sync::SyncManager;
use crate::user_api::types::Blob;

use std::sync::Arc;

use chrono::{DateTime, Utc};

use sqlx::sqlite::SqlitePool;

use tokio::sync::RwLock;
use tonic::transport::Server;
use tonic::transport::server::Router;
use tonic::{Request, Response, Status};

use uuid::Uuid;

pub struct SyncServiceImpl {
    config: P2PConfig,
    db_pool: SqlitePool,
    sync_manager: Arc<RwLock<SyncManager>>,
}

impl SyncServiceImpl {
    pub fn new(
        config: P2PConfig,
        db_pool: SqlitePool,
        sync_manager: Arc<RwLock<SyncManager>>,
    ) -> Self {
        Self {
            config,
            db_pool,
            sync_manager,
        }
    }

    #[tracing::instrument(name = "Handling announce blob request", skip(self))]
    async fn handle_announce(&self, request: AnnounceBlobRequest) -> Result<SyncResponse, Status> {
        let Some(metadata) = request.metadata else {
            return Ok(SyncResponse {
                status: SyncStatus::Error as i32,
                message: "Missing metadata".to_string(),
                response_data: None,
            });
        };

        if self.does_blob_exist(&metadata.id).await? {
            return Ok(SyncResponse {
                status: SyncStatus::AlreadyExists as i32,
                message: "Blob already exists".to_string(),
                response_data: Some(
                    crate::node_api::proto::sync_response::ResponseData::Announce(
                        AnnounceBlobResponse { accepted: false },
                    ),
                ),
            });
        }

        let blob_id: Uuid = metadata
            .id
            .parse()
            .map_err(|_| Status::invalid_argument("Invalid uuid"))?;

        self.sync_manager
            .write()
            .await
            .queue_blob_fetch(blob_id, metadata.hash);

        Ok(SyncResponse {
            status: SyncStatus::Success as i32,
            message: "Blob announcement accepted".to_string(),
            response_data: Some(
                crate::node_api::proto::sync_response::ResponseData::Announce(
                    AnnounceBlobResponse { accepted: true },
                ),
            ),
        })
    }

    #[tracing::instrument(name = "Handling fetch blob request", skip(self))]
    async fn handle_fetch(&self, request: FetchBlobRequest) -> Result<SyncResponse, Status> {
        let blob_id: Uuid = request
            .blob_id
            .parse()
            .map_err(|_| Status::invalid_argument("Invalid uuid"))?;

        let blob_result = self.fetch_blob_by_id(&blob_id).await?;

        let Some(blob) = blob_result else {
            return Ok(SyncResponse {
                status: SyncStatus::NotFound as i32,
                message: format!("Blob not found: {}", request.blob_id),
                response_data: Some(crate::node_api::proto::sync_response::ResponseData::Fetch(
                    FetchBlobResponse { blob: None },
                )),
            });
        };

        let proto_metadata = BlobMetadata {
            id: blob.id.to_string(),
            hash: blob.hash.unwrap_or_default(),
            content_type: blob.content_type,
            size: blob.size,
            owner_id: blob.owner_id.to_string(),
            public_key: blob.public_key,
            metadata: blob.metadata,
            created_at: blob.created_at.to_rfc3339(),
            deleted_at: blob.deleted_at.map(|dt| dt.to_rfc3339()),
        };

        let proto_blob = if request.include_content {
            crate::node_api::proto::Blob {
                metadata: Some(proto_metadata),
                content: blob.content,
            }
        } else {
            crate::node_api::proto::Blob {
                metadata: Some(proto_metadata),
                content: Vec::new(),
            }
        };

        self.mark_blob_status_as_completed(Utc::now(), &blob.id, &request.peer_id)
            .await?;

        Ok(SyncResponse {
            status: SyncStatus::Success as i32,
            message: "Blob fetched successfully".to_string(),
            response_data: Some(crate::node_api::proto::sync_response::ResponseData::Fetch(
                FetchBlobResponse {
                    blob: Some(proto_blob),
                },
            )),
        })
    }

    #[tracing::instrument(name = "Handling delete blob request", skip(self))]
    async fn handle_delete(&self, request: DeleteBlobRequest) -> Result<SyncResponse, Status> {
        let blob_exists = self.does_blob_exist(&request.blob_id).await?;

        if !blob_exists {
            return Ok(SyncResponse {
                status: SyncStatus::NotFound as i32,
                message: format!("Blob not found: {}", request.blob_id),
                response_data: Some(crate::node_api::proto::sync_response::ResponseData::Delete(
                    DeleteBlobResponse { deleted: false },
                )),
            });
        }

        self.mark_blob_as_deleted(&request.blob_id).await?;

        Ok(SyncResponse {
            status: SyncStatus::Success as i32,
            message: "Blob deleted successfully".to_string(),
            response_data: Some(crate::node_api::proto::sync_response::ResponseData::Delete(
                DeleteBlobResponse { deleted: true },
            )),
        })
    }

    #[tracing::instrument(name = "Handling sync status request", skip(self))]
    async fn handle_sync_status(&self, request: SyncStatusRequest) -> Result<SyncResponse, Status> {
        let statuses = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                Option<DateTime<Utc>>,
                Option<DateTime<Utc>>,
            ),
        >(
            "SELECT peer_node_id, blob_id, sync_status, last_sync_attempt, last_successful_sync
             FROM sync_status WHERE blob_id = ?",
        )
        .bind(&request.blob_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let peer_statuses = statuses
            .into_iter()
            .map(
                |(peer_id, blob_id, status, last_attempt, last_success)| PeerSyncStatus {
                    peer_id,
                    blob_id,
                    status,
                    last_sync_attempt: last_attempt.map(|dt| dt.to_rfc3339()),
                    last_successful_sync: last_success.map(|dt| dt.to_rfc3339()),
                },
            )
            .collect();

        Ok(SyncResponse {
            status: SyncStatus::Success as i32,
            message: "Sync status retrieved".to_string(),
            response_data: Some(
                crate::node_api::proto::sync_response::ResponseData::StatusResp(
                    SyncStatusResponse { peer_statuses },
                ),
            ),
        })
    }

    #[tracing::instrument(name = "Handling node info request", skip(self))]
    async fn handle_node_info(&self) -> Result<SyncResponse, Status> {
        let blob_count: i32 =
            sqlx::query_scalar("SELECT COUNT(*) FROM blobs WHERE deleted_at IS NULL")
                .fetch_one(&self.db_pool)
                .await
                .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let storage_used: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(size), 0) FROM blobs WHERE deleted_at IS NULL")
                .fetch_one(&self.db_pool)
                .await
                .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(SyncResponse {
            status: SyncStatus::Success as i32,
            message: "Node info retrieved".to_string(),
            response_data: Some(
                crate::node_api::proto::sync_response::ResponseData::NodeInfo(NodeInfoResponse {
                    node_id: self.config.node_id.clone(),
                    blob_count,
                    storage_used,
                    capabilities: vec!["blob_sync".to_string(), "user_auth".to_string()],
                }),
            ),
        })
    }

    #[tracing::instrument(name = "Handling handshake request", skip(self))]
    async fn handle_handshake(&self, data: HandshakeRequest) -> Result<SyncResponse, Status> {
        self.sync_manager
            .write()
            .await
            .add_peer(data.node_id, &data.node_url)
            .await;

        Ok(SyncResponse {
            status: SyncStatus::Success as i32,
            message: "Handshake successful".to_string(),
            response_data: Some(
                crate::node_api::proto::sync_response::ResponseData::Handshake(HandshakeResponse {
                    status: HandshakeStatus::Acknowledged as i32,
                    peer_id: Some(self.config.node_id.clone()),
                }),
            ),
        })
    }

    #[tracing::instrument(name = "Checking if blob exists", skip(self))]
    async fn does_blob_exist(&self, blob_id: &String) -> Result<bool, Status> {
        let blob_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM blobs WHERE id = ? AND deleted_at IS NULL)",
        )
        .bind(blob_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(blob_exists)
    }

    #[tracing::instrument(
        name = "Fetching blob by ID",
        skip(self, blob_id),
        fields(
            id = %blob_id,
        )
    )]
    async fn fetch_blob_by_id(&self, blob_id: &Uuid) -> Result<Option<Blob>, Status> {
        let blob_result =
            sqlx::query_as::<_, Blob>("SELECT * FROM blobs WHERE id = ? AND deleted_at IS NULL")
                .bind(blob_id)
                .fetch_optional(&self.db_pool)
                .await
                .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(blob_result)
    }

    #[tracing::instrument(name = "Marking blob as deleted", skip(self))]
    async fn mark_blob_as_deleted(&self, blob_id: &String) -> Result<(), Status> {
        let now = Utc::now();
        sqlx::query("UPDATE blobs SET deleted_at = ? WHERE id = ?")
            .bind(now)
            .bind(blob_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(())
    }

    #[tracing::instrument(name = "Mark blob status as completed", skip(self))]
    async fn mark_blob_status_as_completed(
        &self,
        timestamp: DateTime<Utc>,
        blob_id: &Uuid,
        peer_id: &String,
    ) -> Result<(), Status> {
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
        .execute(&self.db_pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(())
    }
}

#[tonic::async_trait]
impl SyncService for SyncServiceImpl {
    async fn sync(&self, request: Request<SyncRequest>) -> Result<Response<SyncResponse>, Status> {
        let response = match request.into_inner().request_type {
            Some(crate::node_api::proto::sync_request::RequestType::Announce(announce)) => {
                self.handle_announce(announce).await?
            }
            Some(crate::node_api::proto::sync_request::RequestType::Fetch(fetch)) => {
                self.handle_fetch(fetch).await?
            }
            Some(crate::node_api::proto::sync_request::RequestType::Delete(delete)) => {
                self.handle_delete(delete).await?
            }
            Some(crate::node_api::proto::sync_request::RequestType::Status(status)) => {
                self.handle_sync_status(status).await?
            }
            Some(crate::node_api::proto::sync_request::RequestType::NodeInfo(_)) => {
                self.handle_node_info().await?
            }
            Some(crate::node_api::proto::sync_request::RequestType::Handshake(data)) => {
                self.handle_handshake(data).await?
            }
            None => {
                return Ok(Response::new(SyncResponse {
                    status: SyncStatus::Error as i32,
                    message: "Invalid request type".to_string(),
                    response_data: None,
                }));
            }
        };

        Ok(Response::new(response))
    }
}

pub async fn create_grpc_server(
    config: P2PConfig,
    db_pool: SqlitePool,
    sync_manager: Arc<RwLock<SyncManager>>,
) -> Result<Router, Status> {
    let service = SyncServiceImpl::new(config, db_pool, sync_manager);

    Ok(Server::builder().add_service(SyncServiceServer::new(service)))
}
