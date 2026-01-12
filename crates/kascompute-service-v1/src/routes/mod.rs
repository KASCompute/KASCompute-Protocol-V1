use axum::Router;
use crate::state::AppState;

pub mod v1;
pub mod legacy;

pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .nest("/v1", v1::router())
        .merge(legacy::router())
}
