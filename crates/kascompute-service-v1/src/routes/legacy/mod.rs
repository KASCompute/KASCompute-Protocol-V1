use axum::Router;
use crate::state::AppState;

mod legacy_handlers;

pub fn router() -> Router<AppState> {
    legacy_handlers::router()
}
