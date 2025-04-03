use crate::node_api::sync::SyncManager;
use crate::user_api::data_loader::{BlobBatcher, BlobLoader, UserBatcher, UserLoader};
use crate::user_api::types::User;

use std::sync::Arc;

use sqlx::{Pool, Sqlite};
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct Context {
    db: Option<Pool<Sqlite>>,
    current_user: Option<User>,
    user_loader: Option<UserLoader>,
    blob_loader: Option<BlobLoader>,
    sync_manager: Option<Arc<RwLock<SyncManager>>>,
    jwt_secret: secrecy::SecretString,
}

impl juniper::Context for Context {}

impl Context {
    pub fn new(
        db: Pool<Sqlite>,
        jwt_secret: secrecy::SecretString,
        p2p: Arc<RwLock<SyncManager>>,
    ) -> Self {
        let user_loader = UserLoader::new(UserBatcher { pool: db.clone() }).with_yield_count(100);
        let blob_loader = BlobLoader::new(BlobBatcher { pool: db.clone() }).with_yield_count(100);

        Self {
            db: Some(db),
            user_loader: Some(user_loader),
            blob_loader: Some(blob_loader),
            current_user: None,
            sync_manager: Some(p2p),
            jwt_secret,
        }
    }

    pub fn with_user(mut self, user: User) -> Self {
        self.current_user = Some(user);
        self
    }

    pub fn current_user(&self) -> Option<&User> {
        self.current_user.as_ref()
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        self.db.as_ref().expect("Database pool not initialized")
    }

    pub fn user_loader(&self) -> &UserLoader {
        self.user_loader
            .as_ref()
            .expect("User loader not initialized")
    }

    pub fn blob_loader(&self) -> &BlobLoader {
        self.blob_loader
            .as_ref()
            .expect("Blob loader not initialized")
    }

    pub fn jwt_secret(&self) -> &secrecy::SecretString {
        &self.jwt_secret
    }

    pub fn sync_manager(&self) -> &Arc<RwLock<SyncManager>> {
        self.sync_manager
            .as_ref()
            .expect("Sync manager not initialized")
    }
}
