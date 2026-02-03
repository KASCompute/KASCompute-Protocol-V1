use axum::{
    extract::{Path, State},
    routing::get,
    Router,
};

use crate::domain::models::{MinerBalanceView, NodeBalanceView, RewardLedgerEntry, RewardView};
use crate::state::AppState;
use crate::util::resp::ok;

pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/rewards/leaderboard", get(leaderboard))
        .route("/rewards/balances", get(balances))
        .route("/rewards/ledger/:miner_id", get(ledger))
        .route("/rewards/nodes/balances", get(node_balances))
}

async fn leaderboard(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let v: Vec<RewardView> = state.rewards_leaderboard().await;
    ok(v)
}

async fn balances(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let v: Vec<MinerBalanceView> = state.rewards_balances().await;
    ok(v)
}

async fn ledger(
    State(state): State<AppState>,
    Path(miner_id): Path<String>,
) -> impl axum::response::IntoResponse {
    let v: Vec<RewardLedgerEntry> = state.rewards_ledger(&miner_id).await;
    ok(v)
}


async fn node_balances(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let v: Vec<NodeBalanceView> = state.rewards_nodes_balances().await;
    ok(v)
}
