use serde::{Deserialize, Serialize};

use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct P2PConfig {
    pub node_id: String,
    pub listen_addr: String,
    pub peers: Vec<String>,
    pub sync_interval_secs: u64,
    pub max_concurrent_syncs: usize,
    pub max_blob_size: usize,
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            node_id: Uuid::new_v4().to_string(),
            listen_addr: "127.0.0.1:50051".to_string(),
            peers: Vec::new(),
            sync_interval_secs: 60,
            max_concurrent_syncs: 5,
            max_blob_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

impl P2PConfig {
    pub fn with_peers(mut self, peers: Vec<String>) -> Self {
        self.peers = peers;
        self
    }

    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = node_id.into();
        self
    }

    pub fn with_listen_addr(mut self, addr: impl Into<String>) -> Self {
        self.listen_addr = addr.into();
        self
    }

    pub fn with_sync_interval(mut self, secs: u64) -> Self {
        self.sync_interval_secs = secs;
        self
    }
}
