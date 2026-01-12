use axum::Router;
use crate::state::AppState;

mod health;
mod nodes;
mod jobs;
mod proofs;
mod mining;
mod rewards;
mod metrics;
mod debug_demo;
mod dashboard;

pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .merge(health::router())
        .merge(nodes::router())
        .merge(jobs::router())
        .merge(proofs::router())
        .merge(mining::router())
        .merge(rewards::router())
        .merge(metrics::router())
        .merge(debug_demo::router())
        .merge(dashboard::router())
}
