//! Integration tests for Phase 9's interactive-session endpoints, against a
//! real router via `tower::ServiceExt::oneshot` — same style as
//! `tests/router.rs`, since each session's game runs on a real dedicated OS
//! thread even under `oneshot` (nothing here is faked or mocked).
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use monopoly_server::{build_router, db, AppState};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tower::ServiceExt;

fn app() -> axum::Router {
    let conn = db::open(":memory:").unwrap();
    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        sessions: Arc::new(Mutex::new(HashMap::new())),
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

/// Polls `GET /sessions/:id` until a decision is pending or the game ends
/// (`step_turn` on the session thread runs many CPU-only turns near
/// instantly, so this rarely spins more than once or twice in practice).
async fn poll_until_settled(app: &axum::Router, id: &str, timeout: Duration) -> Value {
    let deadline = Instant::now() + timeout;
    loop {
        let (status, body) = send(app, get(&format!("/sessions/{id}"))).await;
        assert_eq!(status, StatusCode::OK);
        if !body["pending"].is_null() || !body["game_over"].is_null() || !body["errored"].is_null()
        {
            return body;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for a pending decision or a finished game"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// A simple, safe answer for any `PendingDecision` — buy when offered, pay
/// the jail fine, and decline/abstain everywhere else. Good enough to keep a
/// session moving without needing to reason about board state from the test.
fn default_answer(pending: &Value) -> Value {
    match pending["kind"].as_str().unwrap() {
        "Purchase" => json!({"kind": "Purchase", "buy": true}),
        "JailAction" => json!({"kind": "JailAction", "action": "PayFine"}),
        "Build" => json!({"kind": "Build", "actions": []}),
        "Mortgage" => json!({"kind": "Mortgage", "actions": []}),
        "AuctionBid" => json!({"kind": "AuctionBid", "amount": null}),
        "TradeProposal" => json!({"kind": "TradeProposal", "offer": null}),
        "TradeResponse" => json!({"kind": "TradeResponse", "accept": false}),
        other => panic!("unexpected pending decision kind: {other}"),
    }
}

fn create_session_body(seed: u64) -> Value {
    json!({
        "players": [
            { "name": "Human", "strategy": "human" },
            { "name": "CPU", "strategy": "buy_optimal" },
        ],
        "human_seat": 0,
        "seed": seed,
    })
}

/// Seed 1: with both seats on a decision-independent strategy (dice/card
/// draws don't depend on any strategy's decisions), player 0 enters jail on
/// its 6th turn — found via a one-off search kept in this comment for
/// reproducibility: `Game::new(RuleSet::default(), &[buy_none, buy_none], 1)`
/// has `state().players[0].in_jail` become true after the 11th `step_turn()`
/// call (turns 1,3,5,7,9,11 belong to player 0 in a 2-player game). This
/// holds regardless of which strategies actually occupy the two seats, since
/// movement is dice-driven, not decision-driven.
const JAIL_SEED: u64 = 1;

#[tokio::test]
async fn full_session_lifecycle_purchase_jail_conflict_and_delete() {
    let app = app();

    let (status, created) = send(&app, post("/sessions", create_session_body(JAIL_SEED))).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["human_seat"], 0);
    assert!(created["state"]["players"].as_array().unwrap().len() == 2);

    let mut seen_purchase = false;
    let mut seen_jail = false;
    let mut last_pending: Option<Value> = None;

    for _ in 0..200 {
        let snap = poll_until_settled(&app, &id, Duration::from_secs(10)).await;
        if !snap["game_over"].is_null() {
            break;
        }
        assert!(
            snap["errored"].is_null(),
            "session thread errored: {:?}",
            snap["errored"]
        );
        let pending = snap["pending"].clone();
        assert!(!pending.is_null(), "expected a pending decision");
        assert_eq!(pending["player"], 0, "only the human seat ever blocks");

        match pending["kind"].as_str().unwrap() {
            "Purchase" => seen_purchase = true,
            "JailAction" => seen_jail = true,
            _ => {}
        }

        let answer = default_answer(&pending);
        let (status, _resp) = send(&app, post(&format!("/sessions/{id}/decisions"), answer)).await;
        assert_eq!(status, StatusCode::OK, "answering a decision must succeed");

        if seen_jail {
            last_pending = Some(pending);
            break;
        }
    }

    assert!(seen_purchase, "expected at least one purchase decision");
    assert!(
        seen_jail,
        "expected a jail decision at the fixed seed {JAIL_SEED}"
    );
    let _ = last_pending;

    // A fresh decision is now pending (or the game already ended, in which
    // case there's nothing left to conflict with — but at this fixed seed
    // the game runs on for a long while after one jail visit).
    let snap = poll_until_settled(&app, &id, Duration::from_secs(10)).await;
    if snap["game_over"].is_null() {
        let pending_kind = snap["pending"]["kind"].as_str().unwrap();
        let wrong_kind = if pending_kind == "Purchase" {
            json!({"kind": "JailAction", "action": "PayFine"})
        } else {
            json!({"kind": "Purchase", "buy": true})
        };
        let (status, body) =
            send(&app, post(&format!("/sessions/{id}/decisions"), wrong_kind)).await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "mismatched decision kind must be 409"
        );
        assert!(body["error"].as_str().is_some());
    }

    // Deleting mid-game leaves no zombie session: a subsequent GET/POST is
    // 404, matching an id that never existed.
    let (status, _) = send(&app, delete(&format!("/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send(&app, get(&format!("/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = send(
        &app,
        post(
            &format!("/sessions/{id}/decisions"),
            json!({"kind": "Purchase", "buy": true}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = send(&app, delete(&format!("/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_session_id_is_404_everywhere() {
    let app = app();
    let id = "sess_does_not_exist";

    let (status, _) = send(&app, get(&format!("/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = send(
        &app,
        post(
            &format!("/sessions/{id}/decisions"),
            json!({"kind": "Purchase", "buy": true}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = send(&app, delete(&format!("/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_session_validates_player_count_and_human_seat() {
    let app = app();

    let (status, body) = send(
        &app,
        post(
            "/sessions",
            json!({
                "players": [ { "name": "Solo", "strategy": "human" } ],
                "human_seat": 0,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().is_some());

    let (status, body) = send(
        &app,
        post(
            "/sessions",
            json!({
                "players": [
                    { "name": "Human", "strategy": "human" },
                    { "name": "CPU", "strategy": "buy_optimal" },
                ],
                "human_seat": 5,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().is_some());

    let (status, body) = send(
        &app,
        post(
            "/sessions",
            json!({
                "players": [
                    { "name": "Human", "strategy": "human" },
                    { "name": "CPU", "strategy": "not_a_real_strategy" },
                ],
                "human_seat": 0,
            }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an unknown CPU strategy is a 400, not a 500"
    );
    assert!(body["error"].as_str().is_some());
}

#[tokio::test]
async fn get_board_returns_all_forty_spaces() {
    let app = app();
    let (status, body) = send(&app, get("/board")).await;
    assert_eq!(status, StatusCode::OK);
    let spaces = body.as_array().unwrap();
    assert_eq!(spaces.len(), 40);
    assert_eq!(spaces[0]["kind"], "go");
    assert_eq!(spaces[1]["kind"], "street");
    assert_eq!(spaces[1]["price"], 60);
    assert_eq!(spaces[5]["kind"], "railroad");
}
