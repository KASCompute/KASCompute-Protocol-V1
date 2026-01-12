use axum::{extract::State, routing::get, Router};
use serde_json::json;

use crate::state::{AppState, ACTIVE_WINDOW_SEC, REWARD_WINDOW_SEC};
use crate::util::resp::ok;
use crate::util::time::now_unix;

pub fn router() -> Router<AppState> {
    Router::<AppState>::new().route("/dashboard/overview", get(overview))
}

async fn overview(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let now = now_unix();
    let s = state.inner.read().await;

    let active_nodes_90s = s
        .nodes
        .values()
        .filter(|n| now.saturating_sub(n.last_seen_unix) <= ACTIVE_WINDOW_SEC)
        .count();

    let proofs_window: Vec<_> = s
        .proofs
        .iter()
        .filter(|p| now >= p.timestamp_unix && now - p.timestamp_unix <= REWARD_WINDOW_SEC)
        .collect();

    let mut active_miners_set = std::collections::HashSet::<String>::new();
    for p in &proofs_window {
        if now.saturating_sub(p.timestamp_unix) <= ACTIVE_WINDOW_SEC {
            active_miners_set.insert(p.node_id.clone());
        }
    }
    let active_miners_90s = active_miners_set.len();

    let verified_proofs = proofs_window.iter().filter(|p| p.signature_verified).count();
    let unverified_proofs = proofs_window.len().saturating_sub(verified_proofs);

    let verified_wu: u64 = proofs_window
        .iter()
        .filter(|p| p.signature_verified)
        .map(|p| p.work_units)
        .sum();

    let unverified_wu: u64 = proofs_window
        .iter()
        .filter(|p| !p.signature_verified)
        .map(|p| p.work_units)
        .sum();

    let total_wu = verified_wu + unverified_wu;
    let verified_pct = if total_wu > 0 {
        (verified_wu as f64) * 100.0 / (total_wu as f64)
    } else {
        0.0
    };

    let jobs_completed_window = s
        .jobs
        .values()
        .filter(|j| {
            j.completed_unix
                .is_some_and(|done| now >= done && now - done <= REWARD_WINDOW_SEC)
        })
        .count();

    ok(json!({
        "timestamp": now,
        "windows": {
            "active_window_sec": ACTIVE_WINDOW_SEC,
            "reward_window_sec": REWARD_WINDOW_SEC
        },
        "activity": {
            "active_nodes_90s": active_nodes_90s,
            "active_miners_90s": active_miners_90s,
            "jobs_completed_window": jobs_completed_window,
            "proofs_window": proofs_window.len()
        },
        "verification": {
            "verified_proofs": verified_proofs,
            "unverified_proofs": unverified_proofs,
            "verified_work_units": verified_wu,
            "unverified_work_units": unverified_wu,
            "verified_pct": verified_pct
        }
    }))
}
