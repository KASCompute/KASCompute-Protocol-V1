use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};

use crate::domain::models::{HeartbeatPayload, Node};
use crate::state::AppState;
use crate::util::ip::client_ip_from_headers;
use crate::util::resp::ok;

pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/nodes", get(list_nodes))
        .route("/nodes/heartbeat", post(heartbeat))
}

async fn list_nodes(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let nodes: Vec<Node> = state.list_nodes().await;
    ok(nodes)
}

async fn heartbeat(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<HeartbeatPayload>,
) -> impl axum::response::IntoResponse {
    
    let ip = client_ip_from_headers(&headers, std::net::IpAddr::from([0, 0, 0, 0]));

    let _heartbeat_ts = payload.timestamp_unix;
    let _heartbeat_sig = payload.signature_hex.as_deref();

    state.upsert_node(payload, ip).await;

    ok(serde_json::json!({ "accepted": true }))
}
