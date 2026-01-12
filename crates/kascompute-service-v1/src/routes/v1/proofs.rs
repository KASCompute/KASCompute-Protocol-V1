use axum::{extract::State, routing::get, Router};

use crate::domain::models::ProofRecord;
use crate::state::AppState;
use crate::util::resp::ok;

pub fn router() -> Router<AppState> {
    Router::<AppState>::new().route("/proofs", get(list_proofs))
}

async fn list_proofs(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let v: Vec<ProofRecord> = state.list_recent_proofs(200).await;
    ok(v)
}
