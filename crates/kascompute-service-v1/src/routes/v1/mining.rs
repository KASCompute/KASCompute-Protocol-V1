use axum::{extract::State, routing::get, Router};

use crate::domain::models::MiningStats;
use crate::state::AppState;
use crate::util::resp::ok;

pub fn router() -> Router<AppState> {
    Router::<AppState>::new().route("/mining", get(mining_info))
}

async fn mining_info(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let stats: MiningStats = state.mining_stats().await;
    ok(stats)
}
