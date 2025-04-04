use crate::configuration::{DatabaseSettings, ServerSettings};
use crate::errors::DANodeError;
use crate::node_api::config::P2PConfig;
use crate::node_api::server;
use crate::node_api::sync::SyncManager;
use crate::user_api::context::Context;
use crate::user_api::types::{Claims, User};
use crate::user_api::{Mutation, Query, Schema};

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use sqlx::{Pool, Sqlite, SqlitePool};

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

use tokio::net::TcpListener;
use tokio::sync::RwLock;

use secrecy::ExposeSecret;

use tracing::{error, info};

pub struct Application {
    graphql_port: u16,
    server: Serve<TcpListener, Router, Router>,
}

impl Application {
    pub async fn build(
        configuration: ServerSettings,
        sync_manager: Arc<RwLock<SyncManager>>,
        db_pool: SqlitePool,
    ) -> Result<Self, DANodeError> {
        let client_listener: TcpListener =
            TcpListener::bind(configuration.client_server_endpoint).await?;

        let graphql_port = client_listener.local_addr()?.port();

        let server = run(
            client_listener,
            db_pool,
            sync_manager,
            configuration.jwt_secret,
        )
        .await?;

        Ok(Self {
            graphql_port,
            server,
        })
    }

    pub fn graphql_port(&self) -> u16 {
        self.graphql_port
    }

    pub async fn run_until_stopped(self) -> Result<(), DANodeError> {
        let Application {
            graphql_port,
            server,
        } = self;

        info!(port = graphql_port, "Starting GraphQL server.");

        server.await.map_err(|e| {
            error!(%e, "failed to run the server");
            DANodeError::Io(e)
        })?;

        Ok(())
    }
}

pub struct P2P {
    connection_address: String,
    pub grpc_server: tonic::transport::server::Router,
    pub sync_manager: Arc<RwLock<SyncManager>>,
}

impl P2P {
    pub async fn try_from(config: P2PConfig, db_pool: SqlitePool) -> Result<Self, DANodeError> {
        let sync_manager = Arc::new(RwLock::new(
            SyncManager::new(config.clone(), db_pool.clone()).await,
        ));

        let connection_address = config.listen_addr.clone();

        let grpc_server = server::create_grpc_server(config, db_pool, sync_manager.clone()).await?;

        Ok(Self {
            connection_address,
            grpc_server,
            sync_manager,
        })
    }

    pub async fn run_until_stopped(self) -> Result<(), DANodeError> {
        let P2P {
            connection_address,
            grpc_server,
            ..
        } = self;

        let grpc_socket_address = SocketAddr::from_str(&connection_address)?;

        info!(port = grpc_socket_address.port(), "Starting gRPC server.");

        grpc_server.serve(grpc_socket_address).await?;

        Ok(())
    }
}

pub async fn get_connection_pool(database: &DatabaseSettings) -> Result<Pool<Sqlite>, DANodeError> {
    let connection_pool = SqlitePool::connect(&database.connection_string())
        .await
        .expect("Failed to connect to SQLite.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    Ok(connection_pool)
}

async fn run(
    client_listener: TcpListener,
    pool: Pool<Sqlite>,
    sync_manager: Arc<RwLock<SyncManager>>,
    jwt_secret: secrecy::SecretString,
) -> Result<Serve<TcpListener, Router, Router>, DANodeError> {
    let schema = Schema::new(Query, Mutation, EmptySubscription::new());

    let app = Router::new()
        .route(
            "/graphql",
            on(MethodFilter::GET.or(MethodFilter::POST), graphql_handler),
        )
        .route("/", get(graphiql("/graphql", "/subscriptions")))
        .layer(Extension(Arc::new(schema)))
        .layer(Extension(pool.clone()))
        .layer(Extension(jwt_secret.clone()))
        .layer(Extension(sync_manager.clone()));

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
