use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;
use crate::error::{LocciKVError, Result};
use crate::storage::Storage;

#[derive(Clone)]
struct AppState {
    storage: Arc<dyn Storage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PutRequest {
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GetResponse {
    key: String,
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SuccessResponse {
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListResponse {
    keys: Vec<String>,
    count: usize,
}

// Convert LocciKVError to HTTP response
impl IntoResponse for LocciKVError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            LocciKVError::KeyNotFound(key) => (StatusCode::NOT_FOUND, format!("Key not found: {}", key)),
            LocciKVError::InvalidOperation(msg) => (StatusCode::BAD_REQUEST, msg),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(ErrorResponse {
            error: message,
        });

        (status, body).into_response()
    }
}

pub async fn start_http_server(addr: String, storage: Arc<dyn Storage>) -> Result<()> {
    let state = AppState { storage };

    let app = Router::new()
        .route("/", get(health_check))
        .route("/health", get(health_check))
        .route("/stats", get(get_stats))
        .route("/kv/:key", get(get_key))
        .route("/kv/:key", post(put_key))
        .route("/kv/:key", delete(delete_key))
        .route("/keys", get(list_keys))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await
        .map_err(|e| LocciKVError::Server(format!("Failed to bind to {}: {}", addr, e)))?;

    info!("HTTP server listening on {}", addr);

    axum::serve(listener, app).await
        .map_err(|e| LocciKVError::Server(format!("Server error: {}", e)))?;

    Ok(())
}

async fn health_check() -> Json<SuccessResponse> {
    Json(SuccessResponse {
        message: "Locci KV is running".to_string(),
    })
}

async fn get_stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    let stats = state.storage.stats().await?;
    Ok(Json(serde_json::json!(stats)))
}

async fn get_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<GetResponse>> {
    let value = state.storage.get(key.as_bytes()).await?
        .ok_or_else(|| LocciKVError::KeyNotFound(key.clone()))?;

    let value_str = String::from_utf8(value)
        .map_err(|_| LocciKVError::InvalidOperation("Value is not valid UTF-8".to_string()))?;

    Ok(Json(GetResponse {
        key,
        value: value_str,
    }))
}

async fn put_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<PutRequest>,
) -> Result<Json<SuccessResponse>> {
    state.storage.put(key.as_bytes(), req.value.as_bytes()).await?;

    Ok(Json(SuccessResponse {
        message: format!("Key '{}' stored successfully", key),
    }))
}

async fn delete_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<SuccessResponse>> {
    // Check if key exists
    if !state.storage.exists(key.as_bytes()).await? {
        return Err(LocciKVError::KeyNotFound(key));
    }

    state.storage.delete(key.as_bytes()).await?;

    Ok(Json(SuccessResponse {
        message: format!("Key '{}' deleted successfully", key),
    }))
}

async fn list_keys(State(state): State<AppState>) -> Result<Json<ListResponse>> {
    let keys_bytes = state.storage.list_keys(None).await?;
    
    let keys: Vec<String> = keys_bytes
        .into_iter()
        .filter_map(|k| String::from_utf8(k).ok())
        .collect();

    let count = keys.len();

    Ok(Json(ListResponse { keys, count }))
}
