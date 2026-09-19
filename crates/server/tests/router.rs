//! Router tests against an in-memory SQLite connection, via
//! `tower::ServiceExt::oneshot` — no real network needed.
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use monopoly_engine::{build_single_run_record, PlayerConfig, RuleSet};
use monopoly_server::{build_router, db, AppState};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

fn app() -> axum::Router {
    let conn = db::open(":memory:").unwrap();
    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
    };
    build_router(state)
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
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

fn post(path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

fn get(path: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .body(Body::empty())
        .unwrap()
}

fn delete(path: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(path)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn post_runs_batch_then_list_and_fetch_it() {
    let app = app();

    let (status, batch) = send(
        &app,
        post(
            "/runs/batch",
            json!({
                "rule_set": RuleSet::default(),
                "players": [
                    { "name": "P1", "strategy": "buy_all" },
                    { "name": "P2", "strategy": "buy_none" },
                ],
                "game_count": 5,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(batch["kind"], "batch");
    let id = batch["id"].as_str().unwrap().to_string();
    assert_eq!(batch["per_game_summary"].as_array().unwrap().len(), 5);

    let (status, list) = send(&app, get("/runs?kind=batch")).await;
    assert_eq!(status, StatusCode::OK);
    let rows = list.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], id);
    assert_eq!(rows[0]["games"], 5);

    let (status, list) = send(&app, get("/runs?strategy=buy_all")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    let (status, list) = send(&app, get("/runs?strategy=nonexistent")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 0);

    let (status, fetched) = send(&app, get(&format!("/runs/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched, batch);
}

#[tokio::test]
async fn replay_game_regenerates_a_specific_seed_from_a_batch() {
    let app = app();

    let (_, batch) = send(
        &app,
        post(
            "/runs/batch",
            json!({
                "rule_set": RuleSet::default(),
                "players": [
                    { "name": "P1", "strategy": "buy_good" },
                    { "name": "P2", "strategy": "buy_bad" },
                ],
                "game_count": 3,
            }),
        ),
    )
    .await;
    let id = batch["id"].as_str().unwrap();
    let seed = batch["seeds"][0].as_u64().unwrap();

    let (status, replayed) = send(&app, get(&format!("/runs/{id}/games/{seed}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replayed["seed"], seed);
    assert!(!replayed["events"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn replay_game_rejects_a_seed_not_in_the_batch() {
    let app = app();
    let (_, batch) = send(
        &app,
        post(
            "/runs/batch",
            json!({
                "rule_set": RuleSet::default(),
                "players": [
                    { "name": "P1", "strategy": "buy_good" },
                    { "name": "P2", "strategy": "buy_bad" },
                ],
                "game_count": 2,
            }),
        ),
    )
    .await;
    let id = batch["id"].as_str().unwrap();

    let (status, body) = send(&app, get(&format!("/runs/{id}/games/999999999999"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("not part of batch run"));
}

#[tokio::test]
async fn replay_game_rejects_a_single_run_id() {
    let app = app();
    let record = build_single_run_record(
        RuleSet::default(),
        vec![
            PlayerConfig {
                name: "P1".to_string(),
                strategy: "buy_good".to_string(),
            },
            PlayerConfig {
                name: "P2".to_string(),
                strategy: "buy_bad".to_string(),
            },
        ],
        7,
    )
    .unwrap();
    let (_, archived) = send(&app, post("/runs", serde_json::to_value(&record).unwrap())).await;
    let id = archived["id"].as_str().unwrap();

    let (status, body) = send(&app, get(&format!("/runs/{id}/games/1"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("not a batch run"));
}

#[tokio::test]
async fn post_runs_batch_with_an_unknown_strategy_is_a_400_not_a_500() {
    let app = app();
    let (status, body) = send(
        &app,
        post(
            "/runs/batch",
            json!({
                "rule_set": RuleSet::default(),
                "players": [
                    { "name": "P1", "strategy": "not_a_real_strategy" },
                    { "name": "P2", "strategy": "buy_bad" },
                ],
                "game_count": 5,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().is_some());
}

#[tokio::test]
async fn get_and_delete_of_an_unknown_id_are_404() {
    let app = app();
    let (status, _) = send(&app, get("/runs/run_does_not_exist")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let response = app
        .clone()
        .oneshot(delete("/runs/run_does_not_exist"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_removes_a_run_from_the_archive() {
    let app = app();
    let (_, batch) = send(
        &app,
        post(
            "/runs/batch",
            json!({
                "rule_set": RuleSet::default(),
                "players": [
                    { "name": "P1", "strategy": "buy_all" },
                    { "name": "P2", "strategy": "buy_none" },
                ],
                "game_count": 2,
            }),
        ),
    )
    .await;
    let id = batch["id"].as_str().unwrap().to_string();

    let response = app
        .clone()
        .oneshot(delete(&format!("/runs/{id}")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let (status, _) = send(&app, get(&format!("/runs/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
