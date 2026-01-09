use axum::Router;
use crate::state::AppState;

pub mod v1;
pub mod legacy;

pub fn router() -> Router<AppState> {
    // New professional API under /v1 plus legacy aliases for compatibility.
    Router::new()
        .nest("/v1", v1::router())
        .merge(legacy::router())
}
