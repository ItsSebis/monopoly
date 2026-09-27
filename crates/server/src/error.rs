//! `docs/api.md`'s stated error convention: every error is `{ "error":
//! "message" }` with a plain HTTP status — no richer envelope.
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use monopoly_engine::ConfigError;
use serde_json::json;

pub enum AppError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
    /// 409: the request conflicts with the resource's current state (Phase
    /// 9's `POST /sessions/:id/decisions` when the submitted decision's kind
    /// doesn't match the currently-pending one, or nothing is pending).
    Conflict(String),
    /// 410: the resource used to exist but is now permanently gone (Phase
    /// 9's `POST /sessions/:id/decisions` once the game has already ended —
    /// distinct from 404, since the session id was valid).
    Gone(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            AppError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            AppError::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
            AppError::Conflict(message) => (StatusCode::CONFLICT, message),
            AppError::Gone(message) => (StatusCode::GONE, message),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<ConfigError> for AppError {
    fn from(err: ConfigError) -> Self {
        AppError::BadRequest(err.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}
