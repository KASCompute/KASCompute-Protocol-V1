use axum::{
    extract::{connect_info::ConnectInfo, State},
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use axum::http::StatusCode;

use crate::state::AppState;
use crate::domain::models::DemoStatus;
use crate::util::resp::{ok, err};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/debug/demo/status", get(status))
        .route("/debug/demo/start", post(start))
        .route("/debug/demo/stop", post(stop))
}


async fn status(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    if !addr.ip().is_loopback() {
        return err("forbidden", "demo endpoints are local-only", StatusCode::FORBIDDEN);
    }
    let s: DemoStatus = state.demo_status().await;
    ok(s)
}

async fn start(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    if !addr.ip().is_loopback() {
        return err("forbidden", "demo endpoints are local-only", StatusCode::FORBIDDEN);
    }
    state.demo_clear().await;
    state.demo_spawn(25, 300).await;
    let s: DemoStatus = state.demo_status().await;
    ok(s)
}

async fn stop(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    if !addr.ip().is_loopback() {
        return err("forbidden", "demo endpoints are local-only", StatusCode::FORBIDDEN);
    }
    state.demo_clear().await;
    let s: DemoStatus = state.demo_status().await;
    ok(s)
}
