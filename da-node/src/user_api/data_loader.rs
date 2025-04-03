use crate::user_api::types::{Blob, User};

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::anyhow;

use dataloader::BatchFn;
use dataloader::cached::Loader;

use uuid::Uuid;

pub struct UserBatcher {
    pub(crate) pool: sqlx::Pool<sqlx::Sqlite>,
}

impl BatchFn<Uuid, Result<User, Arc<anyhow::Error>>> for UserBatcher {
    async fn load(&mut self, user_ids: &[Uuid]) -> HashMap<Uuid, Result<User, Arc<anyhow::Error>>> {
        if user_ids.is_empty() {
            return HashMap::new();
        }

        let placeholders = (0..user_ids.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");

        let query = format!("SELECT * FROM users WHERE id IN ({})", placeholders);

        let mut db_query = sqlx::query_as::<_, User>(&query);
        for id in user_ids {
            db_query = db_query.bind(id);
        }

        match db_query.fetch_all(&self.pool).await {
            Ok(users) => {
                let mut map = HashMap::new();
                for user in users {
                    map.insert(user.id, Ok(user));
                }

                for id in user_ids {
                    if !map.contains_key(id) {
                        map.insert(*id, Err(Arc::new(anyhow!("User not found: {}", id))));
                    }
                }

                map
            }
            Err(e) => {
                let error = Arc::new(anyhow!("Database error: {}", e));
                user_ids
                    .iter()
                    .map(|id| (*id, Err(error.clone())))
                    .collect()
            }
        }
    }
}

pub struct BlobBatcher {
    pub(crate) pool: sqlx::Pool<sqlx::Sqlite>,
}

impl BatchFn<Uuid, Result<Blob, Arc<anyhow::Error>>> for BlobBatcher {
    async fn load(&mut self, blob_ids: &[Uuid]) -> HashMap<Uuid, Result<Blob, Arc<anyhow::Error>>> {
        if blob_ids.is_empty() {
            return HashMap::new();
        }

        let placeholders = (0..blob_ids.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");

        let query = format!(
            "SELECT * FROM blobs WHERE id IN ({}) AND deleted_at IS NULL",
            placeholders
        );

        let mut db_query = sqlx::query_as::<_, Blob>(&query);
        for id in blob_ids {
            db_query = db_query.bind(id);
        }

        match db_query.fetch_all(&self.pool).await {
            Ok(blobs) => {
                let mut map = HashMap::new();
                for blob in blobs {
                    map.insert(blob.id, Ok(blob));
                }

                for id in blob_ids {
                    if !map.contains_key(id) {
                        map.insert(*id, Err(Arc::new(anyhow!("Blob not found: {}", id))));
                    }
                }

                map
            }
            Err(e) => {
                let error = Arc::new(anyhow!("Database error: {}", e));
                blob_ids
                    .iter()
                    .map(|id| (*id, Err(error.clone())))
                    .collect()
            }
        }
    }
}

pub type UserLoader = Loader<Uuid, Result<User, Arc<anyhow::Error>>, UserBatcher>;
pub type BlobLoader = Loader<Uuid, Result<Blob, Arc<anyhow::Error>>, BlobBatcher>;
