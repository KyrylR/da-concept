use crate::node_api::config::P2PConfig;
use crate::node_api::proto::sync_service_server::{SyncService, SyncServiceServer};
use crate::node_api::proto::{
    AnnounceBlobRequest, AnnounceBlobResponse, BlobMetadata, DeleteBlobRequest, DeleteBlobResponse,
    FetchBlobRequest, FetchBlobResponse, NodeInfoResponse, PeerSyncStatus, SyncRequest,
    SyncResponse, SyncStatus, SyncStatusRequest, SyncStatusResponse,
};
use crate::node_api::sync::SyncManager;
use crate::user_api::types::Blob;

use std::sync::Arc;

use chrono::{DateTime, Utc};

use sqlx::sqlite::SqlitePool;

use tokio::sync::RwLock;
use tonic::{Request, Response, Status};

use tracing::info;

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

    async fn handle_announce(&self, request: AnnounceBlobRequest) -> Result<SyncResponse, Status> {
        let Some(metadata) = request.metadata else {
            return Ok(SyncResponse {
                status: SyncStatus::Error as i32,
                message: "Missing metadata".to_string(),
                response_data: None,
            });
        };

        let blob_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM blobs WHERE id = ? AND deleted_at IS NULL)",
        )
        .bind(&metadata.id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        if blob_exists {
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

        self.sync_manager
            .write()
            .await
            .queue_blob_fetch(metadata.id, metadata.hash);

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

    async fn handle_fetch(&self, request: FetchBlobRequest) -> Result<SyncResponse, Status> {
        let blob_result =
            sqlx::query_as::<_, Blob>("SELECT * FROM blobs WHERE id = ? AND deleted_at IS NULL")
                .bind(&request.blob_id)
                .fetch_optional(&self.db_pool)
                .await
                .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

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

    async fn handle_delete(&self, request: DeleteBlobRequest) -> Result<SyncResponse, Status> {
        let blob_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM blobs WHERE id = ? AND deleted_at IS NULL)",
        )
        .bind(&request.blob_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        if !blob_exists {
            return Ok(SyncResponse {
                status: SyncStatus::NotFound as i32,
                message: format!("Blob not found: {}", request.blob_id),
                response_data: Some(crate::node_api::proto::sync_response::ResponseData::Delete(
                    DeleteBlobResponse { deleted: false },
                )),
            });
        }

        let now = Utc::now();
        sqlx::query("UPDATE blobs SET deleted_at = ? WHERE id = ?")
            .bind(now)
            .bind(&request.blob_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(SyncResponse {
            status: SyncStatus::Success as i32,
            message: "Blob deleted successfully".to_string(),
            response_data: Some(crate::node_api::proto::sync_response::ResponseData::Delete(
                DeleteBlobResponse { deleted: true },
            )),
        })
    }

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

    type SyncStreamStream = tokio_stream::wrappers::ReceiverStream<Result<SyncResponse, Status>>;

    async fn sync_stream(
        &self,
        request: Request<tonic::Streaming<SyncRequest>>,
    ) -> Result<Response<Self::SyncStreamStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(10);
        let mut stream = request.into_inner();

        tokio::spawn(async move {
            while let Some(req) = stream.message().await.unwrap_or(None) {
                tx.send(Ok(SyncResponse {
                    status: SyncStatus::Error as i32,
                    message: "Stream sync not implemented".to_string(),
                    response_data: None,
                }))
                .await
                .unwrap();
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }
}

pub async fn create_grpc_server(
    config: P2PConfig,
    db_pool: SqlitePool,
    sync_manager: Arc<RwLock<SyncManager>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = config.listen_addr.parse()?;
    let service = SyncServiceImpl::new(config, db_pool, sync_manager);

    info!("Starting gRPC server on {}", addr);

    tonic::transport::Server::builder()
        .add_service(SyncServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
