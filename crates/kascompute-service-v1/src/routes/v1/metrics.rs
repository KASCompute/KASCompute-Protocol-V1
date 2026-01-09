use axum::{routing::get, Router, extract::State};
use crate::state::AppState;
use crate::domain::models::Metrics;
use crate::util::resp::ok;

pub fn router() -> Router<AppState> {
    Router::new().route("/metrics", get(metrics))
}

async fn metrics(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let m: Metrics = state.compute_metrics(300).await;
    ok(m)
}
