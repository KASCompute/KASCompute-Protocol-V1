use axum::{http::StatusCode, Json};
use serde::Serialize;
use serde_json::Value;

use shared_v1::dto::{ApiEnvelope, ApiError};

pub fn ok<T: Serialize>(data: T) -> (StatusCode, Json<ApiEnvelope<T>>) {
    (
        StatusCode::OK,
        Json(ApiEnvelope {
            status: "ok".to_string(),
            data: Some(data),
            error: None,
            ts: crate::util::time::now_unix(),
        }),
    )
}

// legacy: keeps () payload
pub fn err(code: &'static str, message: impl Into<String>) -> (StatusCode, Json<ApiEnvelope<()>>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiEnvelope {
            status: "error".to_string(),
            data: None,
            error: Some(ApiError {
                code: code.to_string(),
                message: message.into(),
            }),
            ts: crate::util::time::now_unix(),
        }),
    )
}

// NEW: error envelope where data is JSON Value (so it matches ok(json!(...)) types)
pub fn err_json(code: &'static str, message: impl Into<String>) -> (StatusCode, Json<ApiEnvelope<Value>>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiEnvelope {
            status: "error".to_string(),
            data: None,
            error: Some(ApiError {
                code: code.to_string(),
                message: message.into(),
            }),
            ts: crate::util::time::now_unix(),
        }),
    )
}

// NEW: same but custom HTTP status
pub fn err_status_json(
    http_status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> (StatusCode, Json<ApiEnvelope<Value>>) {
    (
        http_status,
        Json(ApiEnvelope {
            status: "error".to_string(),
            data: None,
            error: Some(ApiError {
                code: code.to_string(),
                message: message.into(),
            }),
            ts: crate::util::time::now_unix(),
        }),
    )
}

pub fn forbidden(message: impl Into<String>) -> (StatusCode, Json<ApiEnvelope<()>>) {
    (
        StatusCode::FORBIDDEN,
        Json(ApiEnvelope {
            status: "error".to_string(),
            data: None,
            error: Some(ApiError {
                code: "forbidden".to_string(),
                message: message.into(),
            }),
            ts: crate::util::time::now_unix(),
        }),
    )
}
