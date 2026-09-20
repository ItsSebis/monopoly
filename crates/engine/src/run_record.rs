//! The canonical "Run record" shapes from `docs/data-model.md`, minus the
//! server-assigned `id`/`kind`/`created_at` fields Phase 4's archive server
//! adds on ingest. Both the CLI's local `--out` JSON and the server's
//! archive body are built from these same types, so the two can't drift the
//! way the CLI's ad-hoc output structs and the documented archive shape
//! briefly did between Phase 3 and Phase 4.
//!
//! Neither record stores a game's full `GameState` (board ownership, house
//! counts, etc.) — only its `events`/`seeds`. That's the same
//! determinism-as-storage-optimization principle `docs/architecture.md`
//! already applies to batch runs, extended to single runs too: any state can
//! be reconstructed by replaying `events` (single) or re-running the engine
//! with a stored `(rule_set, players, seed)` (batch, via
//! `build_single_run_record`).

use serde::Serialize;

use crate::batch::{run_batch, AggregateStats, BatchGameSummary};
use crate::board::Board;
use crate::config::PlayerConfig;
use crate::events::EventEnvelope;
use crate::game::{ConfigError, Game};
use crate::rules::RuleSet;
use crate::stats::{compute_stats, PerGameStats};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

#[derive(Debug, Clone, Serialize)]
pub struct SingleRunRecord {
    pub rule_set: RuleSet,
    pub players: Vec<PlayerConfig>,
    pub seed: u64,
    pub events: Vec<EventEnvelope>,
    pub final_stats: PerGameStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchRunRecord {
    pub rule_set: RuleSet,
    pub players: Vec<PlayerConfig>,
    pub seeds: Vec<u64>,
    pub per_game_summary: Vec<BatchGameSummary>,
    pub aggregate_stats: AggregateStats,
}

/// Runs one full game and packages it as a `SingleRunRecord` — the
/// self-contained building block the server's `GET /runs/{id}/games/{seed}`
/// replay endpoint calls directly. The CLI's `run` command builds this same
/// struct by hand instead, from a `GameResult` it already produced for its
/// own terminal summary, to avoid simulating the game twice.
pub fn build_single_run_record(
    rules: RuleSet,
    players: Vec<PlayerConfig>,
    seed: u64,
) -> Result<SingleRunRecord, ConfigError> {
    let mut game = Game::new(rules.clone(), &players, seed)?;
    let result = game.run_to_completion();
    let board = Board::standard();
    let final_stats = compute_stats(&board, &players, &rules, &result);
    Ok(SingleRunRecord {
        rule_set: rules,
        players,
        seed,
        events: result.events,
        final_stats,
    })
}

/// Runs a full batch and packages it as a `BatchRunRecord`. A thin wrapper
/// over `run_batch` — used by both the CLI's `batch` command and the
/// server's `POST /runs/batch`. `on_game_done` is forwarded to `run_batch`
/// unchanged (see its own doc comment) — `None` for the server, `Some` for
/// the CLI's progress bar.
pub fn build_batch_run_record(
    rules: RuleSet,
    players: Vec<PlayerConfig>,
    seeds: Vec<u64>,
    on_game_done: Option<&(dyn Fn() + Sync)>,
) -> Result<BatchRunRecord, ConfigError> {
    let result = run_batch(rules.clone(), players.clone(), &seeds, on_game_done)?;
    Ok(BatchRunRecord {
        rule_set: rules,
        players,
        seeds,
        per_game_summary: result.per_game,
        aggregate_stats: result.aggregate,
    })
}

/// Per-game seeds for a batch, deterministically derived from one base seed
/// so the whole batch reproduces from that single printed/stored number.
/// Shared by the CLI and the server so the same base seed always maps to the
/// same per-game seeds regardless of which one dispatched the batch.
pub fn derive_batch_seeds(base_seed: u64, count: usize) -> Vec<u64> {
    let mut rng = StdRng::seed_from_u64(base_seed);
    (0..count).map(|_| rng.gen()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn players(strategies: &[&str]) -> Vec<PlayerConfig> {
        strategies
            .iter()
            .map(|s| PlayerConfig {
                name: s.to_string(),
                strategy: s.to_string(),
            })
            .collect()
    }

    #[test]
    fn derive_batch_seeds_is_deterministic_and_sized() {
        let a = derive_batch_seeds(42, 10);
        let b = derive_batch_seeds(42, 10);
        assert_eq!(a, b);
        assert_eq!(a.len(), 10);
    }

    #[test]
    fn different_base_seeds_produce_different_batches() {
        assert_ne!(derive_batch_seeds(1, 5), derive_batch_seeds(2, 5));
    }

    #[test]
    fn build_single_run_record_matches_the_requested_seed() {
        let record =
            build_single_run_record(RuleSet::default(), players(&["buy_good", "buy_all"]), 7)
                .unwrap();
        assert_eq!(record.seed, 7);
        assert!(!record.events.is_empty());
        assert!(record.final_stats.turns > 0);
    }

    #[test]
    fn build_single_run_record_propagates_config_errors() {
        let err = build_single_run_record(RuleSet::default(), players(&["only_one"]), 1);
        assert!(matches!(err, Err(ConfigError::NotEnoughPlayers(1))));
    }

    #[test]
    fn build_batch_run_record_reports_every_seed() {
        let seeds = derive_batch_seeds(1, 20);
        let record = build_batch_run_record(
            RuleSet::default(),
            players(&["buy_good", "buy_bad"]),
            seeds.clone(),
            None,
        )
        .unwrap();
        assert_eq!(record.seeds, seeds);
        assert_eq!(record.per_game_summary.len(), 20);
        assert_eq!(record.aggregate_stats.games, 20);
    }
}
