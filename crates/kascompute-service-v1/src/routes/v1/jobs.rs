use axum::{routing::{get, post}, Router, extract::{Path, State}, Json};


use crate::domain::models::{NextJobRequest, NextJobResponse, ProofSubmitRequest, JobsSummary, Job, RecentJobView};
use crate::state::AppState;
use crate::util::resp::{ok, err_status_json};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/jobs/next", post(next_job))
        .route("/jobs/:job_id/proof", post(submit_proof))
        .route("/jobs", get(list_jobs))
        .route("/jobs/summary", get(jobs_summary))
        .route("/jobs/recent", get(recent_jobs))
}

async fn next_job(
    State(state): State<AppState>,
    Json(req): Json<NextJobRequest>,
) -> impl axum::response::IntoResponse {
    let lease = state.assign_job(&req.node_id).await;
    ok(NextJobResponse { job: lease })
}

async fn submit_proof(
    Path(job_id): Path<u64>,
    State(state): State<AppState>,
    Json(p): Json<ProofSubmitRequest>,
) -> impl axum::response::IntoResponse {
    match state.complete_job(job_id, p).await {
        Ok(record) => ok(serde_json::json!({"status":"accepted","receipt": record.receipt, "signature_verified": record.signature_verified})),
        Err(code) => err_status_json(axum::http::StatusCode::BAD_REQUEST, code, "proof rejected"),
    }
}

async fn list_jobs(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let jobs: Vec<Job> = state.list_jobs().await;
    ok(jobs)
}

async fn jobs_summary(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let s: JobsSummary = state.summarize_jobs().await;
    ok(s)
}

async fn recent_jobs(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let v: Vec<RecentJobView> = state.list_recent_jobs_with_proofs(50).await;
    ok(v)
}
