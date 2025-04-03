use crate::user_api::context::Context;

use base64::Engine;
use base64::engine::general_purpose;

use juniper::{FieldResult, GraphQLInputObject, GraphQLObject, graphql_object};

use serde::{Deserialize, Serialize};

use chrono::{DateTime, Utc};

use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Blob {
    pub id: Uuid,
    pub content: Vec<u8>,
    pub metadata: Option<String>,
    pub content_type: Option<String>,
    pub size: i32,
    pub hash: Option<String>,
    pub owner_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(GraphQLInputObject)]
pub struct LoginInput {
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(GraphQLInputObject)]
pub struct NewUserInput {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) email: Option<String>,
}

#[derive(GraphQLInputObject)]
pub struct BlobInput {
    pub(crate) content: String,
    pub(crate) metadata: Option<String>,
    pub(crate) content_type: Option<String>,
}

#[derive(GraphQLInputObject)]
pub struct BlobIdInput {
    pub(crate) blob_id: Uuid,
}

#[derive(GraphQLObject)]
#[graphql(context = Context)]
pub struct AuthPayload {
    pub(crate) token: String,
    pub(crate) user: UserSchema,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub(crate) sub: Uuid,
    pub(crate) exp: i64,
    pub(crate) iat: i64,
}

#[derive(Clone, Debug)]
pub struct UserSchema {
    id: Uuid,
    username: String,
    email: Option<String>,
}

#[graphql_object(context = Context)]
impl UserSchema {
    fn id(&self) -> &Uuid {
        &self.id
    }

    fn username(&self) -> &str {
        &self.username
    }

    fn email(&self) -> &Option<String> {
        &self.email
    }

    async fn blobs(&self, context: &Context) -> FieldResult<Vec<BlobSchema>> {
        if context.current_user().is_none() {
            return Err("Authentication required".into());
        }

        let blobs = sqlx::query_as::<_, Blob>(
            "SELECT * FROM blobs WHERE owner_id = ? AND deleted_at IS NULL",
        )
        .bind(self.id)
        .fetch_all(context.pool())
        .await?;

        Ok(blobs.into_iter().map(BlobSchema::new).collect())
    }
}

impl UserSchema {
    pub fn new(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlobSchema {
    id: Uuid,
    metadata: Option<String>,
    content_type: Option<String>,
    size: i32,
    hash: Option<String>,
    owner_id: Uuid,
    created_at: DateTime<Utc>,
}

#[graphql_object(context = Context)]
impl BlobSchema {
    fn id(&self) -> &Uuid {
        &self.id
    }

    fn metadata(&self) -> &Option<String> {
        &self.metadata
    }

    fn content_type(&self) -> &Option<String> {
        &self.content_type
    }

    fn size(&self) -> i32 {
        self.size
    }

    fn hash(&self) -> &Option<String> {
        &self.hash
    }

    fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    async fn owner(&self, context: &Context) -> FieldResult<UserSchema> {
        Ok(UserSchema::new(
            context.user_loader().load(self.owner_id).await?,
        ))
    }

    async fn content(&self, context: &Context) -> FieldResult<String> {
        let blob = sqlx::query_as::<_, Blob>("SELECT * FROM blobs WHERE id = ?")
            .bind(self.id)
            .fetch_one(context.pool())
            .await?;

        Ok(general_purpose::STANDARD.encode(&blob.content))
    }
}

impl BlobSchema {
    pub fn new(blob: Blob) -> Self {
        Self {
            id: blob.id,
            metadata: blob.metadata,
            content_type: blob.content_type,
            size: blob.size,
            hash: blob.hash,
            owner_id: blob.owner_id,
            created_at: blob.created_at,
        }
    }
}
