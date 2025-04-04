pub mod context;
pub mod data_loader;
pub mod types;

use crate::node_api::sync_blob;
use crate::user_api::context::Context;
use crate::user_api::types::{
    AuthPayload, Blob, BlobIdInput, BlobInput, BlobSchema, Claims, LoginInput, NewUserInput, User,
    UserSchema,
};

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};

use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode as encodeJWT};
use juniper::{EmptySubscription, FieldResult, graphql_object};
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};

use uuid::Uuid;

pub struct Query;

#[graphql_object(context = Context)]
impl Query {
    fn api_version() -> &'static str {
        "1.0"
    }

    async fn me(context: &Context) -> FieldResult<Option<UserSchema>> {
        match context.current_user() {
            Some(user) => Ok(Some(UserSchema::new(user.clone()))),
            None => Ok(None),
        }
    }

    async fn user(id: Uuid, context: &Context) -> FieldResult<UserSchema> {
        Ok(UserSchema::new(context.user_loader().load(id).await?))
    }

    async fn blob(id: Uuid, context: &Context) -> FieldResult<BlobSchema> {
        Ok(BlobSchema::new(context.blob_loader().load(id).await?))
    }

    async fn blobs(context: &Context) -> FieldResult<Vec<BlobSchema>> {
        let blobs = sqlx::query_as::<_, Blob>("SELECT * FROM blobs WHERE deleted_at IS NULL")
            .fetch_all(context.pool())
            .await?;

        Ok(blobs.into_iter().map(BlobSchema::new).collect())
    }
}

pub struct Mutation;

fn get_secure_random_bytes() -> [u8; 32] {
    ring::rand::generate(&ring::rand::SystemRandom::new())
        .unwrap()
        .expose()
}

#[graphql_object(context = Context)]
impl Mutation {
    async fn register(input: NewUserInput, context: &Context) -> FieldResult<AuthPayload> {
        let password_hash = Argon2::default()
            .hash_password(
                input.password.as_bytes(),
                &SaltString::encode_b64(&get_secure_random_bytes())?,
            )?
            .to_string();

        let private_key = crypto::PrivateKey::generate();

        let user: User = sqlx::query_as(
            "INSERT INTO users (id, username, password_hash, private_key, email) VALUES (?, ?, ?, ?, ?) RETURNING *"
        )
            .bind(Uuid::new_v4())
            .bind(&input.username)
            .bind(&password_hash)
            .bind(private_key.get_encoded_private_key())
            .bind(&input.email)
            .fetch_one(context.pool())
            .await?;

        let now = Utc::now();
        let claims = Claims {
            sub: user.id,
            exp: (now + Duration::days(7)).timestamp(),
            iat: now.timestamp(),
        };

        let token = encodeJWT(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(context.jwt_secret().expose_secret().as_bytes()),
        )?;

        Ok(AuthPayload {
            token,
            user: UserSchema::new(user),
        })
    }

    async fn login(input: LoginInput, context: &Context) -> FieldResult<AuthPayload> {
        let user: User = sqlx::query_as("SELECT * FROM users WHERE username = ?")
            .bind(&input.username)
            .fetch_optional(context.pool())
            .await?
            .ok_or("User not found")?;

        let parsed_hash = PasswordHash::new(&user.password_hash)?;
        Argon2::default()
            .verify_password(input.password.as_bytes(), &parsed_hash)
            .map_err(|_| "Invalid password")?;

        let now = Utc::now();
        let claims = Claims {
            sub: user.id,
            exp: (now + Duration::days(7)).timestamp(),
            iat: now.timestamp(),
        };

        let token = encodeJWT(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(context.jwt_secret().expose_secret().as_bytes()),
        )?;

        Ok(AuthPayload {
            token,
            user: UserSchema::new(user),
        })
    }

    async fn add_blob(blob_input: BlobInput, context: &Context) -> FieldResult<BlobSchema> {
        let current_user = context.current_user().ok_or("Authentication required")?;

        let private_key = crypto::PrivateKey::try_from(current_user.private_key.as_str())?;
        let related_pubkey = private_key.public_key.get_encoded_public_key();

        let content = crypto::encrypt(blob_input.content.clone(), &private_key);

        let size = content.len() as i64;

        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = format!("{:x}", hasher.finalize());

        let blob: Blob = sqlx::query_as(
            "INSERT INTO blobs (id, content, metadata, content_type, size, hash, owner_id, public_key)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(&content)
        .bind(&blob_input.metadata)
        .bind(&blob_input.content_type)
        .bind(size)
        .bind(&hash)
        .bind(current_user.id)
        .bind(related_pubkey)
        .fetch_one(context.pool())
        .await?;

        sync_blob(
            context.sync_manager(),
            blob.id,
            blob.hash.clone().unwrap_or_default(),
        )
        .await
        .map_err(|e| format!("Failed to sync blob: {}", e))?;

        Ok(BlobSchema::from(blob, blob_input.content))
    }

    async fn delete_blob(input: BlobIdInput, context: &Context) -> FieldResult<bool> {
        let current_user = context.current_user().ok_or("Authentication required")?;

        let blob =
            sqlx::query_as::<_, Blob>("SELECT * FROM blobs WHERE id = ? AND deleted_at IS NULL")
                .bind(input.blob_id)
                .fetch_optional(context.pool())
                .await?
                .ok_or("Blob not found")?;

        if blob.owner_id != current_user.id {
            return Err("You don't have permission to delete this blob".into());
        }

        // Soft delete by setting deleted_at
        let now = Utc::now();
        sqlx::query("UPDATE blobs SET deleted_at = ? WHERE id = ?")
            .bind(now)
            .bind(input.blob_id)
            .execute(context.pool())
            .await?;

        sync_blob(
            context.sync_manager(),
            blob.id,
            blob.hash.unwrap_or_default(),
        )
        .await
        .map_err(|e| format!("Failed to sync blob: {}", e))?;

        sqlx::query(
            "INSERT INTO sync_status (blob_id, peer_node_id, sync_status)
             SELECT ?, peer_node_id, 'pending' FROM sync_status
             WHERE blob_id = ? AND sync_status = 'completed'",
        )
        .bind(input.blob_id)
        .bind(input.blob_id)
        .execute(context.pool())
        .await?;

        Ok(true)
    }
}

pub type Schema = juniper::RootNode<'static, Query, Mutation, EmptySubscription<Context>>;
