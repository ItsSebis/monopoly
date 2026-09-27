//! Integration tests for Phase 9's interactive-session endpoints, against a
//! real router via `tower::ServiceExt::oneshot` (`tests/common`) — same
//! style as `tests/router.rs`. Each session's game runs on a real dedicated
//! OS thread even under `oneshot`; nothing here is faked or mocked.
mod common;

use axum::http::StatusCode;
use common::{app, delete, get, post, send};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

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

async fn create_session(app: &axum::Router, seed: u64) -> String {
    let (status, created) = send(app, post("/sessions", create_session_body(seed))).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["human_seat"], 0);
    created["id"].as_str().unwrap().to_string()
}

/// Seed 1, with a human at seat 0 and `buy_optimal` at seat 1: player 0
/// enters jail on its 6th turn. Jail entry (a `GoToJail` landing, 3 doubles,
/// or a card) is driven purely by dice/card draws, which are a pure
/// function of the seed and independent of any strategy's decisions — but
/// *reaching* turn 11 without an early bankruptcy is not, so this is
/// asserted against the exact seat pairing `create_session_body` sets up,
/// not claimed to hold for any pairing.
const JAIL_SEED: u64 = 1;

#[tokio::test]
async fn purchase_and_jail_decisions_are_asked_and_answered() {
    let app = app();
    let id = create_session(&app, JAIL_SEED).await;

    let mut seen_purchase = false;
    let mut seen_jail = false;

    for _ in 0..200 {
        let snap = poll_until_settled(&app, &id, Duration::from_secs(10)).await;
        assert!(
            snap["errored"].is_null(),
            "session thread errored: {:?}",
            snap["errored"]
        );
        if !snap["game_over"].is_null() {
            break;
        }
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

        if seen_jail && seen_purchase {
            break;
        }
    }

    assert!(seen_purchase, "expected at least one purchase decision");
    assert!(
        seen_jail,
        "expected a jail decision at the fixed seed {JAIL_SEED}"
    );
}

#[tokio::test]
async fn mismatched_kind_decision_is_409() {
    let app = app();
    let id = create_session(&app, JAIL_SEED).await;

    // Turn 1 always asks the human seat something (if not a purchase, the
    // engine still calls `decide_build` unconditionally at end of turn), so
    // this is never null and the check below always runs — not
    // conditionally skipped on a race with the game already being over.
    let snap = poll_until_settled(&app, &id, Duration::from_secs(10)).await;
    let pending_kind = snap["pending"]["kind"]
        .as_str()
        .expect("turn 1 always asks the human seat a decision");
    let wrong_kind = if pending_kind == "Purchase" {
        json!({"kind": "JailAction", "action": "PayFine"})
    } else {
        json!({"kind": "Purchase", "buy": true})
    };

    let (status, body) = send(&app, post(&format!("/sessions/{id}/decisions"), wrong_kind)).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "mismatched decision kind must be 409"
    );
    assert!(body["error"].as_str().is_some());
}

#[tokio::test]
async fn deleting_a_session_makes_subsequent_requests_404() {
    let app = app();
    let id = create_session(&app, JAIL_SEED).await;
    poll_until_settled(&app, &id, Duration::from_secs(10)).await;

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
async fn submitting_a_decision_after_game_over_is_410() {
    let app = app();
    let id = create_session(&app, JAIL_SEED).await;

    // Drive the game to completion with the same safe defaults
    // `purchase_and_jail_decisions_are_asked_and_answered` uses.
    let mut snap = poll_until_settled(&app, &id, Duration::from_secs(10)).await;
    let mut guard = 0;
    while snap["game_over"].is_null() {
        guard += 1;
        assert!(
            guard < 500,
            "game did not end within a generous decision budget"
        );
        assert!(
            snap["errored"].is_null(),
            "session thread errored: {:?}",
            snap["errored"]
        );
        let answer = default_answer(&snap["pending"]);
        send(&app, post(&format!("/sessions/{id}/decisions"), answer)).await;
        snap = poll_until_settled(&app, &id, Duration::from_secs(10)).await;
    }

    let (status, body) = send(
        &app,
        post(
            &format!("/sessions/{id}/decisions"),
            json!({"kind": "Purchase", "buy": true}),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::GONE,
        "no decision can be answered once the game has ended"
    );
    assert!(body["error"].as_str().is_some());
}

#[tokio::test]
async fn since_seq_filtering_only_returns_new_events() {
    let app = app();
    let id = create_session(&app, JAIL_SEED).await;

    let first = poll_until_settled(&app, &id, Duration::from_secs(10)).await;
    let first_seq = first["seq"].as_u64().unwrap();
    assert!(
        first_seq > 0,
        "by the time a decision is pending, at least the roll/move that led to it happened"
    );

    let (status, unchanged) =
        send(&app, get(&format!("/sessions/{id}?since_seq={first_seq}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        unchanged["events"].as_array().unwrap().len(),
        0,
        "nothing new has happened since the watermark we just read"
    );

    let answer = default_answer(&first["pending"]);
    send(&app, post(&format!("/sessions/{id}/decisions"), answer)).await;
    poll_until_settled(&app, &id, Duration::from_secs(10)).await;

    let (status, advanced) =
        send(&app, get(&format!("/sessions/{id}?since_seq={first_seq}"))).await;
    assert_eq!(status, StatusCode::OK);
    let new_events = advanced["events"].as_array().unwrap();
    assert!(
        !new_events.is_empty(),
        "answering a decision and letting the game continue must produce new events"
    );
    assert!(new_events
        .iter()
        .all(|e| e["seq"].as_u64().unwrap() > first_seq));
}

#[tokio::test]
async fn out_of_range_build_index_is_400_not_a_panic() {
    let app = app();
    let id = create_session(&app, JAIL_SEED).await;
    poll_until_settled(&app, &id, Duration::from_secs(10)).await;

    let (status, body) = send(
        &app,
        post(
            &format!("/sessions/{id}/decisions"),
            json!({"kind": "Build", "actions": [{"Build": 99}]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().is_some());

    // The session thread must still be alive and unaffected.
    let (status, snap) = send(&app, get(&format!("/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(snap["errored"].is_null());
}

#[tokio::test]
async fn out_of_range_mortgage_index_is_400_not_a_panic() {
    let app = app();
    let id = create_session(&app, JAIL_SEED).await;
    poll_until_settled(&app, &id, Duration::from_secs(10)).await;

    let (status, body) = send(
        &app,
        post(
            &format!("/sessions/{id}/decisions"),
            json!({"kind": "Mortgage", "actions": [{"Mortgage": 99}]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().is_some());

    let (status, snap) = send(&app, get(&format!("/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(snap["errored"].is_null());
}

#[tokio::test]
async fn out_of_range_trade_proposal_index_is_400_not_a_panic() {
    let app = app();
    let id = create_session(&app, JAIL_SEED).await;
    poll_until_settled(&app, &id, Duration::from_secs(10)).await;

    let (status, body) = send(
        &app,
        post(
            &format!("/sessions/{id}/decisions"),
            json!({
                "kind": "TradeProposal",
                "offer": {
                    "to": 1,
                    "offered_properties": [99],
                    "offered_cash": 0,
                    "requested_properties": [],
                    "requested_cash": 0,
                },
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().is_some());

    let (status, snap) = send(&app, get(&format!("/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(snap["errored"].is_null());
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
async fn create_session_rejects_too_few_players() {
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
}

#[tokio::test]
async fn create_session_rejects_an_out_of_range_human_seat() {
    let app = app();
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
}

#[tokio::test]
async fn create_session_rejects_an_unknown_cpu_strategy() {
    let app = app();
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
