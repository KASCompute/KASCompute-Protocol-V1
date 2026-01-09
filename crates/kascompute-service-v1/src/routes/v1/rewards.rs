use axum::{routing::get, Router, extract::State};
use crate::state::AppState;
use crate::domain::models::RewardView;
use crate::util::resp::ok;

pub fn router() -> Router<AppState> {
    Router::new().route("/rewards/leaderboard", get(leaderboard))
}

async fn leaderboard(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let v: Vec<RewardView> = state.rewards_leaderboard().await;
    ok(v)
}
