//
// Copyright 2026 Hans W. Uhlig, IBM. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, put},
    Json, Router,
};
use clap::Parser;
use nanograph_core::object::{ContainerId, DatabaseId, SecurityPrincipal, TableId, TenantId};
use nanograph_kvm::{KeyValueDatabaseConfig, KeyValueDatabaseManager};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};

/// REST API Server for Nanograph Key-Value Database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Host address to bind to
    #[arg(short = 'H', long, default_value = "127.0.0.1", env = "KVM_HOST")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 8080, env = "KVM_PORT")]
    port: u16,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info", env = "KVM_LOG_LEVEL")]
    log_level: String,
}

/// Application state shared across handlers
#[derive(Clone)]
struct AppState {
    manager: Arc<KeyValueDatabaseManager>,
    principal: SecurityPrincipal,
}

/// Request body for PUT operations
#[derive(Debug, Deserialize)]
struct PutRequest {
    key: String,
    value: String,
}

/// Response body for GET operations
#[derive(Debug, Serialize)]
struct GetResponse {
    key: String,
    value: Option<String>,
}

/// Response body for DELETE operations
#[derive(Debug, Serialize)]
struct DeleteResponse {
    key: String,
    deleted: bool,
}

/// Error response
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

/// Custom error type for API handlers
#[derive(Debug)]
enum ApiError {
    KeyValueError(String),
    InvalidInput(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::KeyValueError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

// Note: KeyValueError is re-exported through nanograph_kvm
impl<T> From<T> for ApiError
where
    T: std::error::Error,
{
    fn from(err: T) -> Self {
        ApiError::KeyValueError(err.to_string())
    }
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// PUT /api/v1/{tenant_id}/{database_id}/{table_id}
/// Store a key-value pair
async fn put_value(
    State(state): State<AppState>,
    Path((tenant_id, database_id, table_id)): Path<(u32, u32, u32)>,
    Json(payload): Json<PutRequest>,
) -> Response {
    let tenant_id = TenantId::from(tenant_id);
    let database_id = DatabaseId::from(database_id);
    let container_id = ContainerId::from_parts(tenant_id, database_id);
    let table_id = TableId::from(table_id);

    let key = payload.key.as_bytes();
    let value = payload.value.as_bytes();

    match state
        .manager
        .put(&state.principal, &container_id, &table_id, key, value)
        .await
    {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({"status": "created"}))).into_response(),
        Err(e) => ApiError::KeyValueError(e.to_string()).into_response(),
    }
}

/// GET /api/v1/{tenant_id}/{database_id}/{table_id}/{key}
/// Retrieve a value by key
async fn get_value(
    State(state): State<AppState>,
    Path((tenant_id, database_id, table_id, key)): Path<(u32, u32, u32, String)>,
) -> Response {
    let tenant_id = TenantId::from(tenant_id);
    let database_id = DatabaseId::from(database_id);
    let container_id = ContainerId::from_parts(tenant_id, database_id);
    let table_id = TableId::from(table_id);

    match state
        .manager
        .get(&state.principal, &container_id, &table_id, key.as_bytes())
        .await
    {
        Ok(result) => {
            let value = result.map(|v| String::from_utf8_lossy(&v).to_string());
            (
                StatusCode::OK,
                Json(GetResponse {
                    key: key.clone(),
                    value,
                }),
            ).into_response()
        }
        Err(e) => ApiError::KeyValueError(e.to_string()).into_response(),
    }
}

/// DELETE /api/v1/{tenant_id}/{database_id}/{table_id}/{key}
/// Delete a key-value pair
async fn delete_value(
    State(state): State<AppState>,
    Path((tenant_id, database_id, table_id, key)): Path<(u32, u32, u32, String)>,
) -> Response {
    let tenant_id = TenantId::from(tenant_id);
    let database_id = DatabaseId::from(database_id);
    let container_id = ContainerId::from_parts(tenant_id, database_id);
    let table_id = TableId::from(table_id);

    match state
        .manager
        .delete(&state.principal, &container_id, &table_id, key.as_bytes())
        .await
    {
        Ok(deleted) => (
            StatusCode::OK,
            Json(DeleteResponse {
                key: key.clone(),
                deleted,
            }),
        ).into_response(),
        Err(e) => ApiError::KeyValueError(e.to_string()).into_response(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing
    let log_level = match args.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    info!("Starting Nanograph KVM Server");

    // Initialize the key-value database manager
    let config = KeyValueDatabaseConfig::default();
    let manager = KeyValueDatabaseManager::new_standalone(config).await?;

    // Create a default security principal (in production, this should be authenticated)
    // For this simple demo server, we create a minimal principal with system-level access
    use nanograph_core::object::{UserId, SystemUserRecord};
    use nanograph_core::types::Timestamp;
    
    let system_user = SystemUserRecord {
        user_id: UserId::from(0),
        username: "system".to_string(),
        enabled: true,
        created_at: Timestamp::now(),
        version: 1,
        last_modified: Timestamp::now(),
        group_ids: vec![],
        role_ids: vec![],
        grants: vec![],
        password_hash: None,
        options: Default::default(),
        metadata: Default::default(),
    };
    
    let principal = SecurityPrincipal::from_system_user(&system_user, &[], &[]);

    let state = AppState {
        manager: Arc::new(manager),
        principal,
    };

    // Build the router
    let app = Router::new()
        .route("/health", get(health_check))
        .route(
            "/api/v1/:tenant_id/:database_id/:table_id",
            put(put_value),
        )
        .route(
            "/api/v1/:tenant_id/:database_id/:table_id/:key",
            get(get_value),
        )
        .route(
            "/api/v1/:tenant_id/:database_id/:table_id/:key",
            delete(delete_value),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start the server
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    info!("Server listening on http://{}", addr);
    info!("Health check: http://{}/health", addr);
    info!("API endpoints:");
    info!("  PUT    /api/v1/{{tenant_id}}/{{database_id}}/{{table_id}}");
    info!("  GET    /api/v1/{{tenant_id}}/{{database_id}}/{{table_id}}/{{key}}");
    info!("  DELETE /api/v1/{{tenant_id}}/{{database_id}}/{{table_id}}/{{key}}");

    axum::serve(listener, app).await?;

    Ok(())
}


