use crate::error::{LocciKVError, Result};
use crate::raft::{Proposal, RaftNode};
use crate::storage::Storage;
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::info;

#[derive(Clone)]
struct AppState {
    storage: Arc<dyn Storage>,
    raft_node: Option<Arc<RaftNode>>,
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

#[derive(Debug, Serialize, Deserialize)]
struct RaftStatusResponse {
    enabled: bool,
    is_leader: bool,
    leader_id: Option<u64>,
}

// Convert LocciKVError to HTTP response
impl IntoResponse for LocciKVError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            LocciKVError::KeyNotFound(key) => {
                (StatusCode::NOT_FOUND, format!("Key not found: {}", key))
            }
            LocciKVError::InvalidOperation(msg) => (StatusCode::BAD_REQUEST, msg),
            LocciKVError::NotLeader(leader_id) => (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("Not leader. Current leader: {:?}", leader_id),
            ),
            LocciKVError::ProposalTimeout => {
                (StatusCode::REQUEST_TIMEOUT, "Proposal timeout".to_string())
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(ErrorResponse { error: message });

        (status, body).into_response()
    }
}

pub async fn start_http_server(
    addr: String,
    storage: Arc<dyn Storage>,
    raft_node: Option<Arc<RaftNode>>,
) -> Result<()> {
    let state = AppState { storage, raft_node };

    let app = Router::new()
        .route("/", get(health_check))
        .route("/health", get(health_check))
        .route("/stats", get(get_stats))
        .route("/raft/status", get(raft_status))
        .route("/kv/:key", get(get_key))
        .route(
            "/kv/:key",
            post(put_key).layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
        )
        .route("/kv/:key", delete(delete_key))
        .route("/keys", get(list_keys))
        .route("/keys/:prefix", get(list_keys_with_prefix))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| LocciKVError::Server(format!("Failed to bind to {}: {}", addr, e)))?;

    info!("HTTP server listening on {}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| LocciKVError::Server(format!("Server error: {}", e)))?;

    Ok(())
}

async fn health_check() -> Json<SuccessResponse> {
    Json(SuccessResponse {
        message: "Locci KV is running".to_string(),
    })
}

async fn raft_status(State(state): State<AppState>) -> Json<RaftStatusResponse> {
    if let Some(raft_node) = &state.raft_node {
        Json(RaftStatusResponse {
            enabled: true,
            is_leader: raft_node.is_leader(),
            leader_id: raft_node.leader_id(),
        })
    } else {
        Json(RaftStatusResponse {
            enabled: false,
            is_leader: false,
            leader_id: None,
        })
    }
}

async fn get_stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    let stats = state.storage.stats().await?;
    Ok(Json(serde_json::json!(stats)))
}

async fn get_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<GetResponse>> {
    // Reads can go directly to storage (linearizable reads would need leader check)
    let value = state
        .storage
        .get(key.as_bytes())
        .await?
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
    // Use Raft if enabled
    if let Some(raft_node) = &state.raft_node {
        // Check if we're the leader
        if !raft_node.is_leader() {
            return Err(LocciKVError::NotLeader(raft_node.leader_id()));
        }

        // Propose through Raft
        let proposal = Proposal::Put {
            key: key.as_bytes().to_vec(),
            value: req.value.as_bytes().to_vec(),
        };

        raft_node.propose(proposal).await?;
    } else {
        // Direct write (Phase 1 mode)
        state
            .storage
            .put(key.as_bytes(), req.value.as_bytes())
            .await?;
    }

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

    // Use Raft if enabled
    if let Some(raft_node) = &state.raft_node {
        // Check if we're the leader
        if !raft_node.is_leader() {
            return Err(LocciKVError::NotLeader(raft_node.leader_id()));
        }

        // Propose through Raft
        let proposal = Proposal::Delete {
            key: key.as_bytes().to_vec(),
        };

        raft_node.propose(proposal).await?;
    } else {
        // Direct delete (Phase 1 mode)
        state.storage.delete(key.as_bytes()).await?;
    }

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

async fn list_keys_with_prefix(
    State(state): State<AppState>,
    Path(prefix): Path<String>,
) -> Result<Json<ListResponse>> {
    let keys_bytes = state.storage.list_keys(Some(prefix.as_bytes())).await?;

    let keys: Vec<String> = keys_bytes
        .into_iter()
        .filter_map(|k| String::from_utf8(k).ok())
        .collect();

    let count = keys.len();

    Ok(Json(ListResponse { keys, count }))
}
