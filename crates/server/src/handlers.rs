//! Route handlers for the six endpoints `docs/api.md` specifies. `POST
//! /runs` accepts either of `monopoly_engine::run_record`'s two shapes as
//! plain JSON (disambiguated by which required fields are present) rather
//! than a typed enum — the engine's stat/event types only ever need to be
//! *produced*, never parsed back, so this avoids adding `Deserialize` to a
//! dozen types purely for a passthrough archive body. `GET /runs/{id}`
//! likewise returns the stored JSON text unchanged rather than round-tripping
//! it through a Rust type.
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use monopoly_engine::{
    build_batch_run_record, build_single_run_record, derive_batch_seeds, PlayerConfig, RuleSet,
    SingleRunRecord,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

use crate::db;
use crate::error::AppError;
use crate::ids::{new_run_id, now_rfc3339};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/runs", get(list_runs).post(create_run))
        .route("/runs/batch", post(create_batch_run))
        .route("/runs/:id", get(get_run).delete(delete_run))
        .route("/runs/:id/games/:seed", get(replay_game))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn create_run(
    State(state): State<AppState>,
    Json(mut body): Json<Value>,
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

    let obj = body
        .as_object_mut()
        .expect("kind_of already confirmed this is a JSON object");
    obj.insert("id".to_string(), Value::String(id.clone()));
    obj.insert("kind".to_string(), Value::String(kind.to_string()));
    obj.insert("created_at".to_string(), Value::String(created_at.clone()));

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
    Json(req): Json<BatchRunRequest>,
) -> Result<Json<Value>, AppError> {
    let strategies = strategies_string(&req.players);

    let record = tokio::task::spawn_blocking(move || -> Result<Value, AppError> {
        let base_seed: u64 = rand::random();
        let seeds = derive_batch_seeds(base_seed, req.game_count);
        let record = build_batch_run_record(req.rule_set, req.players, seeds)?;
        Ok(serde_json::to_value(record)?)
    })
    .await
    .expect("batch simulation task panicked")?;

    let id = new_run_id();
    let created_at = now_rfc3339();
    let mut record = record;
    let obj = record
        .as_object_mut()
        .expect("BatchRunRecord always serializes to a JSON object");
    obj.insert("id".to_string(), Value::String(id.clone()));
    obj.insert("kind".to_string(), Value::String("batch".to_string()));
    obj.insert("created_at".to_string(), Value::String(created_at.clone()));

    let record_text = serde_json::to_string(&record)?;
    insert_record(&state, id, "batch", created_at, strategies, record_text).await?;

    Ok(Json(record))
}

async fn insert_record(
    state: &AppState,
    id: String,
    kind: &'static str,
    created_at: String,
    strategies: String,
    record_text: String,
) -> Result<(), AppError> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.lock().expect("db mutex poisoned");
        db::insert(&conn, &id, kind, &created_at, &strategies, &record_text)
    })
    .await
    .expect("db task panicked")?;
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
    let db = state.db.clone();
    let rows = tokio::task::spawn_blocking(move || {
        let conn = db.lock().expect("db mutex poisoned");
        db::list(&conn, params.kind.as_deref(), params.strategy.as_deref())
    })
    .await
    .expect("db task panicked")?;

    let summaries = rows
        .into_iter()
        .map(|(id, kind, created_at, record)| build_summary(id, kind, created_at, &record))
        .collect::<Result<Vec<_>, AppError>>()?;
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
    Path((id, seed)): Path<(String, u64)>,
) -> Result<Json<SingleRunRecord>, AppError> {
    let record = fetch_record(&state, &id).await?;
    let value: Value = serde_json::from_str(&record)?;

    if value.get("kind").and_then(Value::as_str) != Some("batch") {
        return Err(AppError::BadRequest(format!("run {id} is not a batch run")));
    }
    let seeds: Vec<u64> = serde_json::from_value(value["seeds"].clone())?;
    if !seeds.contains(&seed) {
        return Err(AppError::BadRequest(format!(
            "seed {seed} is not part of batch run {id}"
        )));
    }
    let rule_set: RuleSet = serde_json::from_value(value["rule_set"].clone())?;
    let players: Vec<PlayerConfig> = serde_json::from_value(value["players"].clone())?;

    let single =
        tokio::task::spawn_blocking(move || build_single_run_record(rule_set, players, seed))
            .await
            .expect("replay task panicked")?;
    Ok(Json(single))
}

async fn delete_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let db = state.db.clone();
    let id_for_query = id.clone();
    let deleted = tokio::task::spawn_blocking(move || {
        let conn = db.lock().expect("db mutex poisoned");
        db::delete(&conn, &id_for_query)
    })
    .await
    .expect("db task panicked")?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("no run with id {id}")))
    }
}

async fn fetch_record(state: &AppState, id: &str) -> Result<String, AppError> {
    let db = state.db.clone();
    let id_for_query = id.to_string();
    tokio::task::spawn_blocking(move || {
        let conn = db.lock().expect("db mutex poisoned");
        db::get(&conn, &id_for_query)
    })
    .await
    .expect("db task panicked")?
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
