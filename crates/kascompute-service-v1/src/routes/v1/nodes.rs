use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};

use serde::Serialize;

use crate::domain::models::{HeartbeatPayload, Node};
use crate::state::{AppState, NODE_ONLINE_TTL_SEC};
use crate::util::ip::client_ip_from_headers;
use crate::util::resp::ok;
use crate::util::time::now_unix;

#[derive(Serialize)]
struct NodeView {
    #[serde(flatten)]
    node: Node,

    // overall
    is_online: bool,
    seconds_since_seen: u64,

    // role-specific
    is_node_online: bool,
    is_miner_online: bool,
    seconds_since_node_seen: Option<u64>,
    seconds_since_miner_seen: Option<u64>,
}

pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/nodes", get(list_nodes))
        .route("/nodes/heartbeat", post(heartbeat))
}

async fn list_nodes(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let nodes: Vec<Node> = state.list_nodes().await;
    let now = now_unix();

    let out: Vec<NodeView> = nodes
        .into_iter()
        .map(|n| {
            // overall
            let seconds = now.saturating_sub(n.last_seen_unix);
            let online = seconds <= NODE_ONLINE_TTL_SEC;

            // role-specific
            let node_secs = n.last_seen_node_unix.map(|t| now.saturating_sub(t));
            let miner_secs = n.last_seen_miner_unix.map(|t| now.saturating_sub(t));

            let node_online = node_secs
                .map(|s| s <= NODE_ONLINE_TTL_SEC)
                .unwrap_or(false);

            let miner_online = miner_secs
                .map(|s| s <= NODE_ONLINE_TTL_SEC)
                .unwrap_or(false);

            NodeView {
                node: n,
                is_online: online,
                seconds_since_seen: seconds,
                is_node_online: node_online,
                is_miner_online: miner_online,
                seconds_since_node_seen: node_secs,
                seconds_since_miner_seen: miner_secs,
            }
        })
        .collect();

    ok(out)
}

async fn heartbeat(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<HeartbeatPayload>,
) -> impl axum::response::IntoResponse {
    let ip = client_ip_from_headers(&headers, std::net::IpAddr::from([0, 0, 0, 0]));
    state.upsert_node(payload, ip).await;
    ok(serde_json::json!({ "accepted": true }))
}
