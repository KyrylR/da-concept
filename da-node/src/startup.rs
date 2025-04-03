use crate::configuration::ServerSettings;
use crate::errors::DANodeError;
use crate::node_api::config::P2PConfig;
use crate::node_api::init_p2p;
use crate::node_api::sync::SyncManager;
use crate::user_api::context::Context;
use crate::user_api::types::{Claims, User};
use crate::user_api::{Mutation, Query, Schema};

use std::sync::Arc;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::{MethodFilter, get, on};
use axum::serve::Serve;
use axum::{Extension, Router};

use jsonwebtoken::{DecodingKey, Validation, decode};

use juniper::EmptySubscription;
use juniper_axum::extract::JuniperRequest;
use juniper_axum::graphiql;
use juniper_axum::response::JuniperResponse;
use secrecy::ExposeSecret;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use tracing::error;

pub struct Application {
    graphql_port: u16,
    grpc_port: u16,
    server: Serve<TcpListener, Router, Router>,
}

impl Application {
    pub async fn build(configuration: ServerSettings) -> Result<Self, DANodeError> {
        let connection_pool = get_connection_pool(&configuration.database_url)?;

        let node_listener: TcpListener =
            TcpListener::bind(&configuration.p2p_config.listen_addr).await?;
        let client_listener: TcpListener =
            TcpListener::bind(configuration.client_server_endpoint).await?;

        let grpc_port = node_listener.local_addr()?.port();
        let graphql_port = client_listener.local_addr()?.port();

        drop(node_listener);

        let server = run(
            client_listener,
            connection_pool,
            configuration.p2p_config,
            configuration.jwt_secret,
        )
        .await?;

        Ok(Self {
            graphql_port,
            grpc_port,
            server,
        })
    }

    pub fn grpc_port(&self) -> u16 {
        self.grpc_port
    }

    pub fn graphql_port(&self) -> u16 {
        self.graphql_port
    }

    pub async fn run_until_stopped(self) -> Result<(), DANodeError> {
        self.server.await.map_err(|e| {
            error!(%e, "failed to run the server");
            DANodeError::Io(e)
        })?;

        Ok(())
    }
}

pub fn get_connection_pool(database_url: &String) -> Result<Pool<Sqlite>, DANodeError> {
    SqlitePoolOptions::new()
        .connect_lazy(database_url)
        .map_err(|e| {
            error!(%e, "failed to connect to the database");
            DANodeError::DatabaseConnection(e)
        })
}

async fn run(
    client_listener: TcpListener,
    pool: Pool<Sqlite>,
    p2p_config: P2PConfig,
    jwt_secret: secrecy::SecretString,
) -> Result<Serve<TcpListener, Router, Router>, DANodeError> {
    let schema = Schema::new(Query, Mutation, EmptySubscription::new());

    let p2p = init_p2p(p2p_config, pool.clone()).await?;

    let app = Router::new()
        .route(
            "/graphql",
            on(MethodFilter::GET.or(MethodFilter::POST), graphql_handler),
        )
        .route("/", get(graphiql("/graphql", "/subscriptions")))
        .layer(Extension(Arc::new(schema)))
        .layer(Extension(pool.clone()))
        .layer(Extension(jwt_secret.clone()))
        .layer(Extension(p2p.clone()));

    let server = axum::serve(client_listener, app);

    Ok(server)
}

async fn graphql_handler(
    Extension(schema): Extension<Arc<Schema>>,
    Extension(pool): Extension<Pool<Sqlite>>,
    Extension(jwt_secret): Extension<secrecy::SecretString>,
    Extension(p2p): Extension<Arc<RwLock<SyncManager>>>,
    headers: HeaderMap,
    JuniperRequest(request): JuniperRequest,
) -> impl IntoResponse {
    let mut context = Context::new(pool.clone(), jwt_secret.clone(), p2p.clone());

    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if let Ok(token_data) = decode::<Claims>(
                    token,
                    &DecodingKey::from_secret(jwt_secret.expose_secret().as_bytes()),
                    &Validation::default(),
                ) {
                    if let Ok(user) = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
                        .bind(token_data.claims.sub)
                        .fetch_one(&pool)
                        .await
                    {
                        context = context.with_user(user);
                    }
                }
            }
        }
    }

    JuniperResponse(request.execute(&*schema, &context).await)
}
