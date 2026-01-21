use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

use axum::response::{IntoResponse, Response};

use crate::domain::models::{
    ComputeDagSubmitRequest, DagNextTaskRequest, DagTaskProofSubmitRequest,
    ComputeDagRunCreateResponse, DagTaskLeaseRenewRequest, DagTaskLeaseRenewResponse,
};

use crate::state::AppState;
use crate::util::resp::{err_status_json, ok};

/// ComputeDAG Routes (Protocol V1)
///
/// Endpoints:
/// - POST /v1/dags/submit
/// - GET  /v1/dags
/// - GET  /v1/dags/:dag_id
/// - POST /v1/dags/:dag_id/runs                  (create run)
/// - GET  /v1/runs/:run_id                       (run view)
/// - POST /v1/runs/:run_id/next                  (lease next task)
/// - POST /v1/runs/:run_id/tasks/:task_id/proof  (submit proof)
/// - POST /v1/runs/:run_id/tasks/:task_id/renew  (renew lease)
pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/dags/submit", post(submit_dag))
        .route("/dags", get(list_dags))
        .route("/dags/:dag_id", get(get_dag))
        .route("/dags/:dag_id/runs", post(create_run))
        .route("/runs/:run_id", get(get_run))
        .route("/runs/:run_id/next", post(next_task))
        .route("/runs/:run_id/tasks/:task_id/proof", post(submit_task_proof))
        .route("/runs/:run_id/tasks/:task_id/renew", post(renew_task))
}

async fn submit_dag(
    State(state): State<AppState>,
    Json(req): Json<ComputeDagSubmitRequest>,
) -> Response {
    match state.submit_dag_spec(req.spec).await {
        Ok(resp) => ok(resp).into_response(),
        Err(code) => err_status_json(axum::http::StatusCode::BAD_REQUEST, code, "dag rejected")
            .into_response(),
    }
}

async fn list_dags(State(state): State<AppState>) -> Response {
    ok(state.list_dags().await).into_response()
}

async fn get_dag(Path(dag_id): Path<String>, State(state): State<AppState>) -> Response {
    match state.get_dag_view(&dag_id).await {
        Some(v) => ok(v).into_response(),
        None => err_status_json(axum::http::StatusCode::NOT_FOUND, "dag_not_found", "not found")
            .into_response(),
    }
}

async fn create_run(Path(dag_id): Path<String>, State(state): State<AppState>) -> Response {
    match state.create_dag_run(&dag_id).await {
        Ok(run_id) => ok(ComputeDagRunCreateResponse { run_id }).into_response(),
        Err(code) => err_status_json(axum::http::StatusCode::BAD_REQUEST, code, "run rejected")
            .into_response(),
    }
}

async fn get_run(Path(run_id): Path<String>, State(state): State<AppState>) -> Response {
    match state.get_dag_run_view(&run_id).await {
        Some(v) => ok(v).into_response(),
        None => err_status_json(axum::http::StatusCode::NOT_FOUND, "run_not_found", "not found")
            .into_response(),
    }
}

async fn next_task(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<DagNextTaskRequest>,
) -> Response {
    // returns Option<DagTaskLease>
    let lease = state.assign_next_dag_task(&run_id, &req.node_id).await;

    // Keep your envelope stable: { task: <Option<DagTaskLease>> }
    ok(serde_json::json!({ "task": lease })).into_response()
}

async fn submit_task_proof(
    Path((run_id, task_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(p): Json<DagTaskProofSubmitRequest>,
) -> Response {
    match state.complete_dag_task(&run_id, &task_id, p).await {
        Ok(view) => ok(view).into_response(),
        Err(code) => err_status_json(axum::http::StatusCode::BAD_REQUEST, code, "proof rejected")
            .into_response(),
    }
}

async fn renew_task(
    Path((run_id, task_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(req): Json<DagTaskLeaseRenewRequest>,
) -> Response {
    match state.renew_dag_task_lease(&run_id, &task_id, &req.node_id).await {
        Ok(exp) => ok(DagTaskLeaseRenewResponse {
            run_id,
            task_id,
            lease_expires_unix: exp,
        })
        .into_response(),
        Err(code) => err_status_json(axum::http::StatusCode::BAD_REQUEST, code, "renew rejected")
            .into_response(),
    }
}

