use axum::{routing::get, Router, extract::State};
use crate::state::AppState;
use crate::domain::models::MiningStats;
use crate::util::resp::ok;

pub fn router() -> Router<AppState> {
    Router::new().route("/mining", get(mining_info))
}

async fn mining_info(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let stats: MiningStats = state.mining_stats().await;
    ok(stats)
}
