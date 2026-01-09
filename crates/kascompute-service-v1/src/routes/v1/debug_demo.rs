use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;

use crate::{state::AppState, util::resp::{ok, err_status_json}};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(demo_status))
        .route("/start", post(demo_start))
        .route("/stop", post(demo_stop))
}

async fn demo_status(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    if !addr.ip().is_loopback() {
        return err_status_json(StatusCode::FORBIDDEN, "forbidden", "demo endpoints are localhost-only");
    }
    let v = serde_json::to_value(state.demo_status().await).unwrap();
    ok(v)
}

async fn demo_start(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    if !addr.ip().is_loopback() {
        return err_status_json(StatusCode::FORBIDDEN, "forbidden", "demo endpoints are localhost-only");
    }
    state.demo_clear().await;
    state.demo_spawn(25, 300).await;
    let v = serde_json::to_value(state.demo_status().await).unwrap();
    ok(v)
}

async fn demo_stop(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    if !addr.ip().is_loopback() {
        return err_status_json(StatusCode::FORBIDDEN, "forbidden", "demo endpoints are localhost-only");
    }
    state.demo_clear().await;
    let v = serde_json::to_value(state.demo_status().await).unwrap();
    ok(v)
}
