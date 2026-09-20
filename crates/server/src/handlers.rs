//! Route handlers for the six endpoints `docs/api.md` specifies. `POST
//! /runs` accepts either of `monopoly_engine::run_record`'s two shapes as
//! plain JSON (disambiguated by which required fields are present) rather
//! than a typed enum — the engine's stat/event types only ever need to be
//! *produced*, never parsed back, so this avoids adding `Deserialize` to a
//! dozen types purely for a passthrough archive body. `GET /runs/{id}`
//! likewise returns the stored JSON text unchanged rather than round-tripping
//! it through a Rust type.
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Path, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{async_trait, Json, Router};
use monopoly_engine::{
    build_batch_run_record, build_single_run_record, derive_batch_seeds, PlayerConfig, RuleSet,
    SingleRunRecord,
};
use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

use crate::db;
use crate::error::AppError;
use crate::ids::{new_run_id, now_rfc3339};
use crate::state::AppState;

/// A `Json<T>` extractor whose rejections go through `AppError` instead of
/// axum's default plain-text rejection body, so a malformed/missing-header
/// request body still gets `docs/api.md`'s `{ "error": "message" }` envelope
/// (with 400, not axum's default 415/422) instead of a bare-text response.
struct AppJson<T>(T);

#[async_trait]
impl<S, T> FromRequest<S> for AppJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(AppJson(value)),
            Err(rejection) => Err(AppError::BadRequest(json_rejection_message(rejection))),
        }
    }
}

fn json_rejection_message(rejection: JsonRejection) -> String {
    rejection.body_text()
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/runs", get(list_runs).post(create_run))
        .route("/runs/batch", post(create_batch_run))
        .route("/runs/:id", get(get_run).delete(delete_run))
        .route("/runs/:id/games/:seed", get(replay_game))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Stamps a passthrough archive body (already confirmed to be a JSON object
/// by `kind_of`, or freshly serialized from a `BatchRunRecord`) with the
/// server-assigned fields `docs/data-model.md`'s `Run record` adds on
/// ingest.
fn tag_record(mut record: Value, id: &str, kind: &str, created_at: &str) -> Value {
    let obj = record
        .as_object_mut()
        .expect("record is always a JSON object");
    obj.insert("id".to_string(), Value::String(id.to_string()));
    obj.insert("kind".to_string(), Value::String(kind.to_string()));
    obj.insert(
        "created_at".to_string(),
        Value::String(created_at.to_string()),
    );
    record
}

async fn create_run(
    State(state): State<AppState>,
    AppJson(body): AppJson<Value>,
) -> Result<Json<Value>, AppError> {
    let kind = kind_of(&body).ok_or_else(|| {
        AppError::BadRequest(
            "body must be a single run record (with `seed`/`events`) or a batch run record \
             (with `seeds`/`per_game_summary`)"
                .to_string(),
        )
    })?;
    let strategies = strategies_field(&body)?;
    let id = new_run_id();
    let created_at = now_rfc3339();

    let body = tag_record(body, &id, kind, &created_at);
    let record_text = serde_json::to_string(&body)?;
    insert_record(&state, id, kind, created_at, strategies, record_text).await?;

    Ok(Json(body))
}

#[derive(Deserialize)]
struct BatchRunRequest {
    rule_set: RuleSet,
    players: Vec<PlayerConfig>,
    game_count: usize,
}

async fn create_batch_run(
    State(state): State<AppState>,
    AppJson(req): AppJson<BatchRunRequest>,
) -> Result<Json<Value>, AppError> {
    let strategies = strategies_string(&req.players);

    let record = tokio::task::spawn_blocking(move || -> Result<Value, AppError> {
        let base_seed: u64 = rand::random();
        let seeds = derive_batch_seeds(base_seed, req.game_count);
        let record = build_batch_run_record(req.rule_set, req.players, seeds, None)?;
        Ok(serde_json::to_value(record)?)
    })
    .await
    .expect("batch simulation task panicked")?;

    let id = new_run_id();
    let created_at = now_rfc3339();
    let record = tag_record(record, &id, "batch", &created_at);
    let record_text = serde_json::to_string(&record)?;
    insert_record(&state, id, "batch", created_at, strategies, record_text).await?;

    Ok(Json(record))
}

/// Runs `f` against the shared connection on a blocking-safe thread pool
/// thread — every DB access goes through this, since `rusqlite::Connection`
/// is synchronous and touching it directly on the async executor would
/// block it.
async fn with_db<T, F>(state: &AppState, f: F) -> T
where
    F: FnOnce(&Connection) -> T + Send + 'static,
    T: Send + 'static,
{
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.lock().expect("db mutex poisoned");
        f(&conn)
    })
    .await
    .expect("db task panicked")
}

async fn insert_record(
    state: &AppState,
    id: String,
    kind: &'static str,
    created_at: String,
    strategies: String,
    record_text: String,
) -> Result<(), AppError> {
    with_db(state, move |conn| {
        db::insert(conn, &id, kind, &created_at, &strategies, &record_text)
    })
    .await?;
    Ok(())
}

#[derive(Deserialize)]
struct ListParams {
    kind: Option<String>,
    strategy: Option<String>,
}

async fn list_runs(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<Value>>, AppError> {
    // Building each summary parses the full stored record's JSON, which for
    // a large batch (thousands of `per_game_summary` entries) is real CPU
    // work — done here, inside the same blocking task as the DB read, rather
    // than after `.await` on the async executor thread.
    let summaries = with_db(&state, move |conn| -> Result<Vec<Value>, AppError> {
        let rows = db::list(conn, params.kind.as_deref(), params.strategy.as_deref())?;
        rows.into_iter()
            .map(|(id, kind, created_at, record)| build_summary(id, kind, created_at, &record))
            .collect()
    })
    .await?;
    Ok(Json(summaries))
}

/// The lightweight preview `docs/api.md` asks `GET /runs` for: id, kind,
/// created_at, the run's config, and just enough of the result to be useful
/// (a single run's winner, or a batch's game count) — not the full body.
fn build_summary(
    id: String,
    kind: String,
    created_at: String,
    record: &str,
) -> Result<Value, AppError> {
    let record: Value = serde_json::from_str(record)?;
    let mut summary = json!({
        "id": id,
        "kind": kind,
        "created_at": created_at,
        "rule_set": record.get("rule_set").cloned().unwrap_or(Value::Null),
        "players": record.get("players").cloned().unwrap_or(Value::Null),
    });
    if kind == "single" {
        let winner = record
            .get("final_stats")
            .and_then(|fs| fs.get("winner"))
            .cloned()
            .unwrap_or(Value::Null);
        summary["winner"] = winner;
    } else {
        let games = record
            .get("aggregate_stats")
            .and_then(|a| a.get("games"))
            .cloned()
            .unwrap_or(Value::Null);
        summary["games"] = games;
    }
    Ok(summary)
}

async fn get_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let record = fetch_record(&state, &id).await?;
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        record,
    )
        .into_response())
}

async fn replay_game(
    State(state): State<AppState>,
    Path((id, seed)): Path<(String, String)>,
) -> Result<Json<SingleRunRecord>, AppError> {
    // Parsed by hand rather than via `Path<(String, u64)>`: a non-numeric
    // `seed` segment would otherwise fail axum's own path deserialization,
    // which rejects with a plain-text body rather than `docs/api.md`'s
    // `{ "error": "message" }` envelope.
    let seed: u64 = seed
        .parse()
        .map_err(|_| AppError::BadRequest(format!("seed {seed} is not a valid u64")))?;

    // Fetching, parsing the (potentially large, for a big batch) stored JSON,
    // validating it, and re-running the game are all done inside one
    // `spawn_blocking` — none of that CPU/DB work belongs on the async
    // executor thread, and there's no reason to hop back to it in between.
    let db = state.db.clone();
    let id_for_query = id.clone();
    let single = tokio::task::spawn_blocking(move || -> Result<SingleRunRecord, AppError> {
        let record_text = {
            let conn = db.lock().expect("db mutex poisoned");
            db::get(&conn, &id_for_query)?
        };
        let record_text = record_text
            .ok_or_else(|| AppError::NotFound(format!("no run with id {id_for_query}")))?;
        let value: Value = serde_json::from_str(&record_text)?;

        if value.get("kind").and_then(Value::as_str) != Some("batch") {
            return Err(AppError::BadRequest(format!(
                "run {id_for_query} is not a batch run"
            )));
        }
        let seeds: Vec<u64> = serde_json::from_value(value["seeds"].clone())?;
        if !seeds.contains(&seed) {
            return Err(AppError::BadRequest(format!(
                "seed {seed} is not part of batch run {id_for_query}"
            )));
        }
        let rule_set: RuleSet = serde_json::from_value(value["rule_set"].clone())?;
        let players: Vec<PlayerConfig> = serde_json::from_value(value["players"].clone())?;
        Ok(build_single_run_record(rule_set, players, seed)?)
    })
    .await
    .expect("replay task panicked")?;
    Ok(Json(single))
}

async fn delete_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let id_for_query = id.clone();
    let deleted = with_db(&state, move |conn| db::delete(conn, &id_for_query)).await?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("no run with id {id}")))
    }
}

async fn fetch_record(state: &AppState, id: &str) -> Result<String, AppError> {
    let id_for_query = id.to_string();
    with_db(state, move |conn| db::get(conn, &id_for_query))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no run with id {id}")))
}

fn kind_of(body: &Value) -> Option<&'static str> {
    let obj = body.as_object()?;
    if obj.contains_key("seed") && obj.contains_key("events") {
        Some("single")
    } else if obj.contains_key("seeds") && obj.contains_key("per_game_summary") {
        Some("batch")
    } else {
        None
    }
}

fn strategies_field(body: &Value) -> Result<String, AppError> {
    let players = body
        .get("players")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::BadRequest("missing or invalid `players` array".to_string()))?;
    let ids = players
        .iter()
        .map(|p| {
            p.get("strategy").and_then(Value::as_str).ok_or_else(|| {
                AppError::BadRequest("each player needs a `strategy` string".to_string())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!(",{},", ids.join(",")))
}

fn strategies_string(players: &[PlayerConfig]) -> String {
    format!(
        ",{},",
        players
            .iter()
            .map(|p| p.strategy.as_str())
            .collect::<Vec<_>>()
            .join(",")
    )
}
