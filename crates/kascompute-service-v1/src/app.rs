use axum::{routing::get, Router};
use axum::response::Redirect;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::state::AppState;

pub fn build_app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any);

    let dashboard_root: PathBuf =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dashboard-pro");

    let dashboard_service = ServeDir::new(dashboard_root.clone())
        .append_index_html_on_directories(true)
        .not_found_service(ServeFile::new(dashboard_root.join("index.html")));


    Router::new()
        .route("/", get(|| async { Redirect::permanent("/dashboard/") }))
        .route("/dashboard", get(|| async { Redirect::permanent("/dashboard/") }))
        .nest_service("/dashboard/", dashboard_service)
        .merge(crate::routes::router())
        .layer(cors)
        .with_state(state)
}
