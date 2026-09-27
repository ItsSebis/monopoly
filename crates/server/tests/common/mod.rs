//! Shared test scaffolding for `tests/router.rs` and `tests/interactive.rs`:
//! a router over an in-memory SQLite connection and empty session map, and
//! thin `Request<Body>` builders, driven via `tower::ServiceExt::oneshot` —
//! no real network needed for either the archive or the interactive-session
//! endpoints.
#![allow(dead_code)] // not every test file uses every helper here.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use monopoly_server::{build_router, db, AppState};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

pub fn app() -> axum::Router {
    let conn = db::open(":memory:").unwrap();
    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        sessions: Arc::new(Mutex::new(HashMap::new())),
    };
    build_router(state)
}

pub async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

pub fn post(path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

pub fn get(path: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .body(Body::empty())
        .unwrap()
}

pub fn delete(path: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(path)
        .body(Body::empty())
        .unwrap()
}
