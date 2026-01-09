use axum::{
    extract::{connect_info::ConnectInfo, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use std::net::SocketAddr;

use crate::domain::models::{HeartbeatPayload, Node};
use crate::state::AppState;
use crate::util::ip::client_ip_from_headers;
use crate::util::resp::ok;
use axum::http::StatusCode;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/nodes", get(list_nodes))
        .route("/nodes/heartbeat", post(heartbeat))
}

async fn list_nodes(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let nodes: Vec<Node> = state.list_nodes().await;
    ok(nodes)
}

async fn heartbeat(
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Json(payload): Json<HeartbeatPayload>,
) -> impl axum::response::IntoResponse {
    // if behind proxy, use forwarded header
    let ip = client_ip_from_headers(&headers, addr.ip());
    state.upsert_node(payload, ip).await;
    ok(serde_json::json!({"accepted": true}))
}
