use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::domain::models::{HeartbeatPayload, NextJobRequest, ProofSubmitRequest};
use crate::state::AppState;
use crate::util::time::now_unix;

pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        // old public
        .route("/health", get(|| async { "OK" }))
        .route("/node/heartbeat", post(heartbeat_old))
        .route("/nodes", get(nodes_old))
        .route("/jobs/next", post(next_job_old))
        .route("/jobs/proof", post(proof_old))
        .route("/jobs", get(jobs_old))
        .route("/jobs/summary", get(summary_old))
        .route("/jobs/recent", get(recent_old))
        .route("/proofs", get(proofs_old))
        .route("/mining", get(mining_old))
        .route("/rewards/leaderboard", get(rewards_old))
        // api mirror (legacy)
        .route("/api/health", get(api_health_old))
        .route("/api/nodes", get(nodes_old))
        .route("/api/jobs", get(jobs_old))
        .route("/api/jobs/summary", get(summary_old))
        .route("/api/jobs/recent", get(recent_old))
        .route("/api/proofs", get(proofs_old))
        .route("/api/mining", get(mining_old))
        .route("/api/rewards/leaderboard", get(rewards_old))
        .route("/api/metrics", get(metrics_old))
}

async fn heartbeat_old(
    State(state): State<AppState>,
    Json(payload): Json<HeartbeatPayload>,
) -> StatusCode {
    // legacy heartbeat doesn't carry real client ip here; state will geo_lookup only if coords missing
    // fallback to 127.0.0.1; real deployments should use /v1/nodes/heartbeat which is proxy-aware.
    state
        .upsert_node(payload, "127.0.0.1".parse().unwrap())
        .await;
    StatusCode::OK
}

async fn nodes_old(State(state): State<AppState>) -> Json<Vec<crate::domain::models::Node>> {
    Json(state.list_nodes().await)
}

async fn next_job_old(
    State(state): State<AppState>,
    Json(req): Json<NextJobRequest>,
) -> Json<serde_json::Value> {
    let lease = state.assign_job(&req.node_id).await;
    if let Some(job) = lease {
        Json(serde_json::json!({ "id": job.id, "work_units": job.work_units }))
    } else {
        Json(serde_json::json!({ "id": null, "work_units": null }))
    }
}

#[derive(Debug, Deserialize)]
struct ProofOldPayload {
    node_id: String,
    job_id: u64,
    work_units: u64,
    #[serde(default)]
    workload_mode: Option<String>,
    #[serde(default)]
    elapsed_ms: Option<u64>,
    #[serde(default)]
    result_hash: Option<String>,
    #[serde(default)]
    client_version: Option<String>,
    #[serde(default)]
    timestamp_unix: Option<u64>,
    #[serde(default)]
    signature_hex: Option<String>,
}

async fn proof_old(
    State(state): State<AppState>,
    Json(p): Json<ProofOldPayload>,
) -> (StatusCode, Json<serde_json::Value>) {
    let req = ProofSubmitRequest {
        node_id: p.node_id.clone(),
        work_units: p.work_units,

        // ✅ v1.1: legacy payload has no miner_id -> fallback to node_id
        miner_id: Some(p.node_id),

        workload_mode: p.workload_mode,
        elapsed_ms: p.elapsed_ms,
        result_hash: p.result_hash,
        client_version: p.client_version,
        timestamp_unix: p.timestamp_unix,
        signature_hex: p.signature_hex,
        proof_hash_hex: None,
        public_key_hex: None,
    };

    match state.complete_job(p.job_id, req).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "status": "error", "message": e })),
        ),
    }
}

async fn jobs_old(State(state): State<AppState>) -> Json<Vec<crate::domain::models::Job>> {
    Json(state.list_jobs().await)
}

async fn summary_old(State(state): State<AppState>) -> Json<crate::domain::models::JobsSummary> {
    Json(state.summarize_jobs().await)
}

async fn recent_old(
    State(state): State<AppState>,
) -> Json<Vec<crate::domain::models::RecentJobView>> {
    Json(state.list_recent_jobs_with_proofs(50).await)
}

async fn proofs_old(State(state): State<AppState>) -> Json<Vec<crate::domain::models::ProofRecord>> {
    Json(state.list_recent_proofs(200).await)
}

async fn mining_old(State(state): State<AppState>) -> Json<crate::domain::models::MiningStats> {
    Json(state.mining_stats().await)
}

async fn rewards_old(State(state): State<AppState>) -> Json<Vec<crate::domain::models::RewardView>> {
    Json(state.rewards_leaderboard().await)
}

#[derive(Serialize)]
struct ApiHealth {
    ok: bool,
    uptime_sec: u64,
    nodes_total: usize,
    nodes_online_90s: usize,
    jobs_total: usize,
    jobs_pending: u64,
    jobs_running: u64,
    jobs_completed: u64,
    block_height: u64,
    month_index: u64,
    total_emitted_nano: u64,
    timestamp: u64,
}

async fn api_health_old(State(state): State<AppState>) -> Json<ApiHealth> {
    let now = now_unix();
    let s = state.inner.read().await;

    let nodes_total = s.nodes.len();
    let nodes_online_90s = s
        .nodes
        .values()
        .filter(|n| now.saturating_sub(n.last_seen_unix) <= 90)
        .count();

    let sum = {
        let mut pending = 0u64;
        let mut running = 0u64;
        let mut completed = 0u64;
        for j in s.jobs.values() {
            match j.status {
                crate::domain::models::JobStatus::Pending => pending += 1,
                crate::domain::models::JobStatus::Running => running += 1,
                crate::domain::models::JobStatus::Completed => completed += 1,
                crate::domain::models::JobStatus::Expired => {}
            }
        }
        (pending, running, completed)
    };

    let uptime = now.saturating_sub(s.genesis_time);

    Json(ApiHealth {
        ok: true,
        uptime_sec: uptime,
        nodes_total,
        nodes_online_90s,
        jobs_total: s.jobs.len(),
        jobs_pending: sum.0,
        jobs_running: sum.1,
        jobs_completed: sum.2,
        block_height: s.block_height,
        month_index: crate::util::time::months_since(s.genesis_time, now),
        total_emitted_nano: s.total_emitted_nano,
        timestamp: now,
    })
}

async fn metrics_old(State(state): State<AppState>) -> Json<crate::domain::models::Metrics> {
    Json(state.compute_metrics(300).await)
}
