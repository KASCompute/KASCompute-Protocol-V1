use axum::{extract::State, routing::get, Router};

use crate::domain::models::RewardView;
use crate::state::AppState;
use crate::util::resp::ok;

pub fn router() -> Router<AppState> {
    Router::<AppState>::new().route("/rewards/leaderboard", get(leaderboard))
}

async fn leaderboard(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let v: Vec<RewardView> = state.rewards_leaderboard().await;
    ok(v)
}
