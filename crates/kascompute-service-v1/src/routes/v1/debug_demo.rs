use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Router,
};
use std::net::{IpAddr, Ipv4Addr};

use crate::state::AppState;
use crate::util::ip::client_ip_from_headers;
use crate::util::resp::{ok, err_status_json};

pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/status", get(demo_status))
        .route("/start", post(demo_start))
        .route("/stop", post(demo_stop))
	.route("/ip", get(debug_ip))
}

fn is_local(headers: &HeaderMap) -> bool {
    let ip = client_ip_from_headers(headers, IpAddr::V4(Ipv4Addr::LOCALHOST));
    ip.is_loopback()
}

async fn demo_status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    if !is_local(&headers) {
        return err_status_json(StatusCode::FORBIDDEN, "forbidden", "demo endpoints are localhost-only");
    }
    let v = serde_json::to_value(state.demo_status().await).unwrap();
    ok(v)
}

async fn demo_start(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    if !is_local(&headers) {
        return err_status_json(StatusCode::FORBIDDEN, "forbidden", "demo endpoints are localhost-only");
    }
    state.demo_clear().await;
    state.demo_spawn(25, 300).await;
    let v = serde_json::to_value(state.demo_status().await).unwrap();
    ok(v)
}

async fn demo_stop(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    if !is_local(&headers) {
        return err_status_json(StatusCode::FORBIDDEN, "forbidden", "demo endpoints are localhost-only");
    }
    state.demo_clear().await;
    let v = serde_json::to_value(state.demo_status().await).unwrap();
    ok(v)
}

async fn debug_ip(headers: HeaderMap) -> impl axum::response::IntoResponse {
    let picked = client_ip_from_headers(&headers, IpAddr::V4(Ipv4Addr::LOCALHOST));

    let v = serde_json::json!({
        "picked_ip": picked.to_string(),
        "x_forwarded_for": headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()),
        "x_real_ip": headers.get("x-real-ip").and_then(|v| v.to_str().ok()),
        "forwarded": headers.get("forwarded").and_then(|v| v.to_str().ok()),
        "cf_connecting_ip": headers.get("cf-connecting-ip").and_then(|v| v.to_str().ok()),
        "true_client_ip": headers.get("true-client-ip").and_then(|v| v.to_str().ok()),
    });

    ok(v)
}
