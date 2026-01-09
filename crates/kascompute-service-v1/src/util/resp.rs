use axum::http::StatusCode;
use axum::Json;
use shared_v1::dto::{ApiEnvelope, ApiError};
use crate::util::time::now_unix;

pub fn ok<T: serde::Serialize>(data: T) -> (StatusCode, Json<ApiEnvelope<T>>) {
    (
        StatusCode::OK,
        Json(ApiEnvelope {
            status: "ok".to_string(),
            data: Some(data),
            error: None,
            ts: now_unix(),
        }),
    )
}

pub fn err(code: &str, message: &str, status: StatusCode) -> (StatusCode, Json<ApiEnvelope<serde_json::Value>>) {
    (
        status,
        Json(ApiEnvelope {
            status: "error".to_string(),
            data: None,
            error: Some(ApiError {
                code: code.to_string(),
                message: message.to_string(),
            }),
            ts: now_unix(),
        }),
    )
}
