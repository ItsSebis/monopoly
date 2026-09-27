//! The Phase 9 interactive-session routes: `POST/GET/DELETE /sessions*` and
//! `GET /board`. Kept separate from `handlers.rs`'s archive endpoints (which
//! are unchanged) and merged into the same router in `handlers::build_router`.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use monopoly_engine::{EventEnvelope, GameResult, PlayerConfig, RuleSet};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::handlers::AppJson;
use crate::ids::new_session_id;
use crate::interactive::board::{board_dto, BoardSpaceDto};
use crate::interactive::decision::{DecisionAnswer, PendingDecision};
use crate::interactive::session::{spawn_session, SessionHandle, SubmitError};
use crate::state::AppState;
use monopoly_engine::state::GameState;

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
    game_over: Option<GameResult>,
    errored: Option<String>,
}

fn snapshot(
    id: &str,
    human_seat: usize,
    handle: &SessionHandle,
    since_seq: u64,
) -> SessionSnapshot {
    debug_assert_eq!(id, handle.id);
    let locked = handle.shared.lock().unwrap();
    SessionSnapshot {
        id: id.to_string(),
        human_seat,
        state: locked.state.clone(),
        pending: locked.pending.clone(),
        events: locked
            .events
            .iter()
            .filter(|e| e.seq > since_seq)
            .cloned()
            .collect(),
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
    if req.players.len() < 2 {
        return Err(AppError::BadRequest(format!(
            "need at least 2 players, got {}",
            req.players.len()
        )));
    }
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

    let snap = snapshot(&id, handle.human_seat, &handle, 0);
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
    Ok(Json(snapshot(
        &id,
        handle.human_seat,
        &handle,
        params.since_seq,
    )))
}

async fn submit_decision(
    State(state): State<AppState>,
    Path(id): Path<String>,
    AppJson(answer): AppJson<DecisionAnswer>,
) -> Result<Json<SessionSnapshot>, AppError> {
    let handle = lookup(&state, &id)?;
    match handle.submit_answer(answer) {
        Ok(()) => Ok(Json(snapshot(&id, handle.human_seat, &handle, 0))),
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
