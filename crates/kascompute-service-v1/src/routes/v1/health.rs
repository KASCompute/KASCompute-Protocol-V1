use axum::{routing::get, Router};
use crate::state::AppState;
use crate::util::resp::ok;
use serde::Serialize;
use crate::util::time::now_unix;

#[derive(Debug, Serialize)]
struct HealthData {
    ok: bool,
    timestamp: u64,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

async fn health() -> impl axum::response::IntoResponse {
    ok(HealthData { ok: true, timestamp: now_unix() })
}
