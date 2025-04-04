use crate::errors::DANodeError;

use crate::node_api::proto;
use crate::node_api::proto::sync_service_client::SyncServiceClient;
use crate::node_api::proto::{
    AnnounceBlobRequest, BlobMetadata, DeleteBlobRequest, FetchBlobRequest, HandshakeRequest,
    HandshakeResponse, NodeInfoRequest, SyncRequest, SyncStatusRequest,
};

use std::sync::Arc;

use tokio::sync::RwLock;
use tonic::transport::Channel;

use uuid::Uuid;

type GrpcClient = SyncServiceClient<Channel>;

#[derive(Clone, Debug)]
pub struct PeerClient {
    peer_url: String,
    client: Arc<RwLock<Option<GrpcClient>>>,
}

impl PeerClient {
    pub fn new(peer_url: String) -> Self {
        Self {
            peer_url,
            client: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn connect(&self) -> Result<(), DANodeError> {
        let peer_url = self.peer_url.clone();
        let client = SyncServiceClient::connect(peer_url).await;

        let mut write_guard = self.client.write().await;
        *write_guard = Some(client?);

        Ok(())
    }

    async fn get_client(&self) -> Result<GrpcClient, DANodeError> {
        {
            let read_guard = self.client.read().await;

            if let Some(client) = read_guard.clone() {
                return Ok(client);
            }
        }

        self.connect().await?;

        let Some(read_guard) = self.client.read().await.clone() else {
            return Err(DANodeError::ClientNotConnected(self.peer_url.clone()));
        };

        Ok(read_guard)
    }

    pub async fn handshake(
        &self,
        node_id: &str,
        node_url: &str,
    ) -> Result<HandshakeResponse, DANodeError> {
        let mut client = self.get_client().await?;

        let request = SyncRequest {
            request_type: Some(proto::sync_request::RequestType::Handshake(
                HandshakeRequest {
                    node_id: node_id.to_owned(),
                    node_url: format!("http://{}", node_url),
                },
            )),
        };

        let response = client.sync(request).await?;

        match response.into_inner().response_data {
            Some(proto::sync_response::ResponseData::Handshake(data)) => Ok(data),
            _ => Err(DANodeError::InvalidResponse(
                "Invalid response from handshake".to_string(),
            )),
        }
    }

    pub async fn announce_blob(&self, metadata: BlobMetadata) -> Result<bool, DANodeError> {
        let mut client = self.get_client().await?;

        let request = SyncRequest {
            request_type: Some(proto::sync_request::RequestType::Announce(
                AnnounceBlobRequest {
                    metadata: Some(metadata),
                },
            )),
        };

        let response = client.sync(request).await?;

        match response.into_inner().response_data {
            Some(proto::sync_response::ResponseData::Announce(announce)) => Ok(announce.accepted),
            _ => Ok(false),
        }
    }

    pub async fn fetch_blob(
        &self,
        node_id: &str,
        blob_id: Uuid,
        include_content: bool,
    ) -> Result<Option<proto::Blob>, DANodeError> {
        let mut client = self.get_client().await?;

        let request = SyncRequest {
            request_type: Some(proto::sync_request::RequestType::Fetch(FetchBlobRequest {
                blob_id: blob_id.to_string(),
                peer_id: node_id.to_owned(),
                include_content,
            })),
        };

        let response = client.sync(request).await?;

        match response.into_inner().response_data {
            Some(proto::sync_response::ResponseData::Fetch(fetch)) => Ok(fetch.blob),
            _ => Ok(None),
        }
    }

    pub async fn delete_blob(&self, blob_id: String) -> Result<bool, DANodeError> {
        let mut client = self.get_client().await?;

        let request = SyncRequest {
            request_type: Some(proto::sync_request::RequestType::Delete(
                DeleteBlobRequest { blob_id },
            )),
        };

        let response = client.sync(request).await?;

        match response.into_inner().response_data {
            Some(proto::sync_response::ResponseData::Delete(delete)) => Ok(delete.deleted),
            _ => Ok(false),
        }
    }

    pub async fn get_node_info(&self) -> Result<Option<proto::NodeInfoResponse>, DANodeError> {
        let mut client = self.get_client().await?;

        let request = SyncRequest {
            request_type: Some(proto::sync_request::RequestType::NodeInfo(
                NodeInfoRequest {},
            )),
        };

        let response = client.sync(request).await?;

        match response.into_inner().response_data {
            Some(proto::sync_response::ResponseData::NodeInfo(info)) => Ok(Some(info)),
            _ => Ok(None),
        }
    }

    pub async fn check_sync_status(
        &self,
        blob_id: String,
    ) -> Result<Option<proto::SyncStatusResponse>, DANodeError> {
        let mut client = self.get_client().await?;

        let request = SyncRequest {
            request_type: Some(proto::sync_request::RequestType::Status(
                SyncStatusRequest { blob_id },
            )),
        };

        let response = client.sync(request).await?;

        match response.into_inner().response_data {
            Some(proto::sync_response::ResponseData::StatusResp(status)) => Ok(Some(status)),
            _ => Ok(None),
        }
    }
}
