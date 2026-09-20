//! `wasm-bindgen` bindings around `engine` for the browser (`docs/roadmap.md`
//! Phase 5). Thin glue only: every method delegates directly to `Game` and
//! serializes with `serde_json` (plain JSON strings, not `JsValue`, so this
//! stays one dependency lighter and data still moves as the same JSON text
//! every other consumer — CLI files, server bodies — already uses). No
//! rules logic lives here or in the browser; this crate only exists to cross
//! the wasm boundary.

use monopoly_engine::{ConfigError, Game, GameConfig};
use wasm_bindgen::prelude::*;

// Named distinctly from wasm-bindgen's own generated default-export module
// loader (conventionally imported as `init` in JS) to avoid two same-named
// but unrelated "init" concepts in the same module.
#[wasm_bindgen(start)]
fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct WasmGame {
    inner: Game,
}

#[wasm_bindgen]
impl WasmGame {
    /// `config_json` is a `GameConfig` (`{ rules, players }`, `rules`
    /// optional) — the same shape `monopoly run --config` and the server's
    /// `POST /runs/batch` already accept, so the browser's config form
    /// produces exactly what every other caller does.
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str, seed: u64) -> Result<WasmGame, JsError> {
        let config: GameConfig =
            serde_json::from_str(config_json).map_err(|e| JsError::new(&e.to_string()))?;
        let inner = Game::new(config.rules, &config.players, seed)
            .map_err(|e: ConfigError| JsError::new(&e.to_string()))?;
        Ok(WasmGame { inner })
    }

    /// Advances the game by exactly one player's turn and returns that
    /// turn's events as a JSON array (`EventEnvelope[]`).
    pub fn step_turn(&mut self) -> String {
        serde_json::to_string(self.inner.step_turn()).expect("events always serialize")
    }

    /// The current `GameState` as JSON — the board renderer's per-turn
    /// authoritative re-sync (see `docs/frontend.md`).
    pub fn state(&self) -> String {
        serde_json::to_string(self.inner.state()).expect("state always serializes")
    }

    pub fn is_over(&self) -> bool {
        self.inner.is_over()
    }
}

/// The registered strategy ids, as a JSON string array — for the browser's
/// strategy dropdown.
#[wasm_bindgen]
pub fn strategy_ids() -> String {
    serde_json::to_string(monopoly_engine::STRATEGY_IDS).expect("strategy ids always serialize")
}

/// The same fallback turn cap `Game::run_to_completion` applies when
/// `RuleSet.max_turns` isn't set. `step_turn` itself enforces no cap at all
/// (see its doc comment) — the browser's live-playback loop needs this to
/// stop a game exactly the way every other caller does, since driving the
/// game one turn at a time is the one path that doesn't go through
/// `run_to_completion`.
#[wasm_bindgen]
pub fn safety_max_turns() -> u32 {
    monopoly_engine::SAFETY_MAX_TURNS
}

/// Deterministically re-simulates `(config, seed)` and returns the canonical
/// `SingleRunRecord` JSON (`docs/data-model.md#run-record`) — the same shape
/// the server archives and the same function `GET /runs/{id}/games/{seed}`
/// calls natively. Lets the browser build the exact archive body for "save
/// this run" (Phase 6) from just the inputs it already has, with no need to
/// capture/replay its own live event stream.
#[wasm_bindgen]
pub fn build_single_run_record(config_json: &str, seed: u64) -> Result<String, JsError> {
    let config: GameConfig =
        serde_json::from_str(config_json).map_err(|e| JsError::new(&e.to_string()))?;
    let record = monopoly_engine::build_single_run_record(config.rules, config.players, seed)
        .map_err(|e: ConfigError| JsError::new(&e.to_string()))?;
    serde_json::to_string(&record).map_err(|e| JsError::new(&e.to_string()))
}
