//! The Phase 9 interactive-session routes: `POST/GET/DELETE /sessions*` and
//! `GET /board`. Kept separate from `handlers.rs`'s archive endpoints (which
//! are unchanged) and merged into the same router in `handlers::build_router`.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use monopoly_engine::state::GameState;
use monopoly_engine::{EventEnvelope, PlayerConfig, RuleSet};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::handlers::AppJson;
use crate::ids::new_session_id;
use crate::interactive::board::{board_dto, BoardSpaceDto};
use crate::interactive::decision::{DecisionAnswer, PendingDecision};
use crate::interactive::session::{spawn_session, GameOver, SessionHandle, SubmitError};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/board", get(get_board))
        .route("/sessions", post(create_session))
        .route("/sessions/:id", get(get_session).delete(delete_session))
        .route("/sessions/:id/decisions", post(submit_decision))
}

async fn get_board() -> Json<Vec<BoardSpaceDto>> {
    Json(board_dto())
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    #[serde(default)]
    rules: RuleSet,
    players: Vec<PlayerConfig>,
    human_seat: usize,
    #[serde(default)]
    seed: Option<u64>,
}

#[derive(Serialize)]
struct SessionSnapshot {
    id: String,
    human_seat: usize,
    state: GameState,
    pending: Option<PendingDecision>,
    events: Vec<EventEnvelope>,
    seq: u64,
    game_over: Option<GameOver>,
    errored: Option<String>,
}

/// Builds a snapshot response from `handle`'s current state, `events`
/// trimmed to `seq > since_seq`. `events` is seq-ordered, so this is a
/// binary search (`partition_point`) rather than a linear scan — matters
/// once a session's full log runs into the thousands of events over a long
/// game.
fn snapshot(handle: &SessionHandle, since_seq: u64) -> SessionSnapshot {
    let locked = handle.shared.lock().unwrap();
    let first_new = locked.events.partition_point(|e| e.seq <= since_seq);
    SessionSnapshot {
        id: handle.id.clone(),
        human_seat: handle.human_seat,
        state: locked.state.clone(),
        pending: locked.pending.clone(),
        events: locked.events[first_new..].to_vec(),
        seq: locked.seq,
        game_over: locked.game_over.clone(),
        errored: locked.errored.clone(),
    }
}

fn lookup(state: &AppState, id: &str) -> Result<Arc<SessionHandle>, AppError> {
    state
        .sessions
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or_else(|| AppError::NotFound(format!("no session with id {id}")))
}

async fn create_session(
    State(state): State<AppState>,
    AppJson(req): AppJson<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionSnapshot>), AppError> {
    // Only `human_seat` is checked here: "at least 2 players" and "every
    // non-human strategy id is recognized" are both already enforced by
    // `Game::with_strategies`'s own `ConfigError` (via `spawn_session`),
    // which maps to the same 400 either way — no need for a second copy of
    // either check up here.
    if req.human_seat >= req.players.len() {
        return Err(AppError::BadRequest(format!(
            "human_seat {} is out of range for {} players",
            req.human_seat,
            req.players.len()
        )));
    }
    let id = new_session_id();
    let seed = req.seed.unwrap_or_else(rand::random);
    let handle = spawn_session(id.clone(), req.rules, req.players, req.human_seat, seed)?;

    let snap = snapshot(&handle, 0);
    state.sessions.lock().unwrap().insert(id, handle);
    Ok((StatusCode::CREATED, Json(snap)))
}

#[derive(Deserialize)]
struct SinceSeqParams {
    #[serde(default)]
    since_seq: u64,
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<SinceSeqParams>,
) -> Result<Json<SessionSnapshot>, AppError> {
    let handle = lookup(&state, &id)?;
    handle.touch();
    Ok(Json(snapshot(&handle, params.since_seq)))
}

async fn submit_decision(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<SinceSeqParams>,
    AppJson(answer): AppJson<DecisionAnswer>,
) -> Result<Json<SessionSnapshot>, AppError> {
    let handle = lookup(&state, &id)?;
    // Defense in depth: the engine itself already treats an out-of-range
    // board-space index as a safe no-op (`Game::try_build`/`try_mortgage`/
    // `trade_is_valid`), but rejecting it here with a clear 400 — before it
    // even reaches the session's channel — is much better API UX than a
    // silently-ignored action.
    if let Err(message) = answer.validate() {
        return Err(AppError::BadRequest(message));
    }
    match handle.submit_answer(answer) {
        Ok(()) => Ok(Json(snapshot(&handle, params.since_seq))),
        Err(SubmitError::NothingPending) => {
            Err(AppError::Conflict("no decision is pending".to_string()))
        }
        Err(SubmitError::KindMismatch) => Err(AppError::Conflict(
            "submitted decision kind does not match the pending decision".to_string(),
        )),
        Err(SubmitError::Finished) => Err(AppError::Gone("the game has already ended".to_string())),
    }
}

async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let removed = state.sessions.lock().unwrap().remove(&id);
    if removed.is_some() {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("no session with id {id}")))
    }
}
