//! The parallel batch runner (`docs/roadmap.md` Phase 3): one game per seed,
//! folded into win-rate, head-to-head, ROI, and distribution statistics
//! (`docs/analysis-and-metrics.md`'s batch-only metrics). Per
//! `docs/architecture.md`'s determinism-as-storage-optimization, only each
//! game's `(seed, winner, turns)` summary survives the call — the full event
//! log and the bulk of `PerGameStats` are dropped per game (see
//! `GameContribution`), so peak memory stays proportional to the batch size
//! rather than to total turns played across it, and anything dropped can be
//! regenerated later from `(RuleSet, players, seed)`.

use std::collections::BTreeMap;

use rayon::prelude::*;
use serde::Serialize;

use crate::board::{Board, BOARD_SIZE};
use crate::config::PlayerConfig;
use crate::game::{ConfigError, Game};
use crate::rules::RuleSet;
use crate::stats::{compute_stats, PropertyRoi};

#[derive(Debug, Clone, Serialize)]
pub struct BatchGameSummary {
    pub seed: u64,
    pub winner: Option<usize>,
    pub turns: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct HeadToHead {
    pub wins: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct DistributionSummary {
    pub mean: f64,
    pub median: f64,
    pub p10: f64,
    pub p90: f64,
}

impl DistributionSummary {
    fn from_values(values: &mut [f64]) -> Self {
        if values.is_empty() {
            return DistributionSummary::default();
        }
        values.sort_by(f64::total_cmp);
        let percentile = |p: f64| values[(((values.len() - 1) as f64) * p).round() as usize];
        DistributionSummary {
            mean: values.iter().sum::<f64>() / values.len() as f64,
            median: percentile(0.5),
            p10: percentile(0.1),
            p90: percentile(0.9),
        }
    }
}

/// Every metric here is keyed by `PlayerConfig::strategy` (the registered
/// strategy id) — a batch mixing two different `strategy_params` under the
/// same id is aggregated together, matching Phase 1-2's config model where
/// no built-in strategy takes per-instance parameters yet.
#[derive(Debug, Clone, Serialize)]
pub struct AggregateStats {
    pub games: usize,
    /// Wins per *seat*, not per game: a strategy occupying two of four seats
    /// is counted twice per game in the denominator, so its rate stays
    /// comparable to a strategy holding a single seat.
    pub win_rate_by_strategy: BTreeMap<String, f64>,
    /// `head_to_head[a][b]`: how often the player running `a` beat the
    /// player running `b` in games where one of the two of them won (a game
    /// won by a third strategy doesn't resolve this specific pairing, so it
    /// isn't counted either way — see `aggregate`). The diagonal
    /// `head_to_head[a][a]` only exists when two seats share a strategy, and
    /// is 50% by construction: each mirror pairing records both a win and a
    /// loss in the same cell, so its `total` counts each such game twice.
    pub head_to_head: BTreeMap<String, BTreeMap<String, HeadToHead>>,
    /// Total rent collected divided by total cost basis, across every
    /// property each strategy owned at the end of its games.
    pub roi_by_strategy: BTreeMap<String, f64>,
    pub game_length: Vec<u32>,
    pub bankruptcy_turns: Vec<u32>,
    pub dice_roll_counts: [u64; 11],
    /// A `Vec` rather than a fixed array — see `stats::PerGameStats::landing_counts`.
    pub landing_counts: Vec<u64>,
    pub final_net_worth_by_strategy: BTreeMap<String, DistributionSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchResult {
    pub per_game: Vec<BatchGameSummary>,
    pub aggregate: AggregateStats,
}

/// Everything `aggregate` needs from one game's `PerGameStats` — extracted
/// immediately after `compute_stats` returns so the much larger
/// `net_worth_by_turn` timeline (one heap-allocated row per turn) and the
/// rest of `PerGameStats` can be dropped before the next game runs. Its size
/// is bounded by the board and player count, unlike `PerGameStats`, which is
/// `O(turns)`.
struct GameContribution {
    dice_roll_counts: [u64; 11],
    landing_counts: Vec<u64>,
    bankruptcy_turns: Vec<u32>,
    property_roi: Vec<PropertyRoi>,
    /// The last `net_worth_by_turn` row — final net worth per player.
    final_net_worth: Vec<u32>,
}

/// A numerator/denominator pair folded across the batch, one entry per
/// strategy: wins over seats played, and rent collected over cost basis.
#[derive(Debug, Clone, Copy, Default)]
struct Ratio {
    numerator: f64,
    denominator: f64,
}

impl Ratio {
    fn rate(self) -> f64 {
        self.numerator / self.denominator
    }
}

/// Runs one full game per seed, in parallel, and folds each into a shared
/// aggregate. Games are independent (no shared mutable state), so this
/// parallelizes trivially across `seeds`.
pub fn run_batch(
    rules: RuleSet,
    players: Vec<PlayerConfig>,
    seeds: &[u64],
) -> Result<BatchResult, ConfigError> {
    // Validated once up front so a config mistake fails the whole batch
    // immediately rather than only once rayon gets around to that seed.
    Game::new(rules.clone(), &players, 0)?;

    let board = Board::standard();
    let per_game: Vec<(BatchGameSummary, GameContribution)> = seeds
        .par_iter()
        .map(|&seed| {
            let mut game =
                Game::new(rules.clone(), &players, seed).expect("already validated above");
            let result = game.run_to_completion();
            let stats = compute_stats(&board, &players, &rules, &result);
            let summary = BatchGameSummary {
                seed,
                winner: result.winner,
                turns: result.turns,
            };
            let contribution = GameContribution {
                dice_roll_counts: stats.dice_roll_counts,
                landing_counts: stats.landing_counts,
                bankruptcy_turns: stats.bankruptcies.iter().map(|b| b.turn).collect(),
                property_roi: stats.property_roi,
                final_net_worth: stats.net_worth_by_turn.last().cloned().unwrap_or_default(),
            };
            (summary, contribution)
        })
        .collect();

    let aggregate = aggregate(&players, &per_game);
    let per_game = per_game.into_iter().map(|(summary, _)| summary).collect();
    Ok(BatchResult {
        per_game,
        aggregate,
    })
}

fn aggregate(
    players: &[PlayerConfig],
    per_game: &[(BatchGameSummary, GameContribution)],
) -> AggregateStats {
    let n = players.len();
    let strategies: Vec<&str> = players.iter().map(|p| p.strategy.as_str()).collect();

    let mut win_rates: BTreeMap<String, Ratio> = BTreeMap::new();
    let mut head_to_head: BTreeMap<String, BTreeMap<String, HeadToHead>> = BTreeMap::new();
    let mut rois: BTreeMap<String, Ratio> = BTreeMap::new();
    let mut game_length = Vec::with_capacity(per_game.len());
    let mut bankruptcy_turns = Vec::new();
    let mut dice_roll_counts = [0u64; 11];
    let mut landing_counts = vec![0u64; BOARD_SIZE];
    let mut final_net_worth: BTreeMap<String, Vec<f64>> = BTreeMap::new();

    for (summary, contribution) in per_game {
        game_length.push(summary.turns);
        for (total, count) in dice_roll_counts
            .iter_mut()
            .zip(contribution.dice_roll_counts)
        {
            *total += count;
        }
        for (total, count) in landing_counts.iter_mut().zip(&contribution.landing_counts) {
            *total += count;
        }
        bankruptcy_turns.extend(contribution.bankruptcy_turns.iter().copied());

        for roi in &contribution.property_roi {
            if let Some(owner) = roi.owner {
                let entry = rois.entry(strategies[owner].to_string()).or_default();
                entry.numerator += roi.rent_collected as f64;
                entry.denominator += roi.cost_basis.max(1) as f64;
            }
        }

        for (player, &net_worth) in contribution.final_net_worth.iter().enumerate() {
            final_net_worth
                .entry(strategies[player].to_string())
                .or_default()
                .push(net_worth as f64);
        }

        for (player, strategy) in strategies.iter().enumerate() {
            let entry = win_rates.entry(strategy.to_string()).or_default();
            entry.denominator += 1.0;
            if summary.winner == Some(player) {
                entry.numerator += 1.0;
            }
        }

        for a in 0..n {
            for b in (a + 1)..n {
                let (winner, loser) = match summary.winner {
                    Some(w) if w == a => (a, b),
                    Some(w) if w == b => (b, a),
                    _ => continue, // a third strategy won: doesn't resolve this pairing
                };
                let cell = head_to_head
                    .entry(strategies[winner].to_string())
                    .or_default()
                    .entry(strategies[loser].to_string())
                    .or_default();
                cell.wins += 1;
                cell.total += 1;
                head_to_head
                    .entry(strategies[loser].to_string())
                    .or_default()
                    .entry(strategies[winner].to_string())
                    .or_default()
                    .total += 1;
            }
        }
    }

    let win_rate_by_strategy = win_rates
        .into_iter()
        .map(|(strategy, ratio)| (strategy, ratio.rate()))
        .collect();

    let roi_by_strategy = rois
        .into_iter()
        .map(|(strategy, ratio)| (strategy, ratio.rate()))
        .collect();

    let final_net_worth_by_strategy = final_net_worth
        .into_iter()
        .map(|(strategy, mut values)| (strategy, DistributionSummary::from_values(&mut values)))
        .collect();

    AggregateStats {
        games: per_game.len(),
        win_rate_by_strategy,
        head_to_head,
        roi_by_strategy,
        game_length,
        bankruptcy_turns,
        dice_roll_counts,
        landing_counts,
        final_net_worth_by_strategy,
    }
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
    fn buy_all_clearly_beats_buy_none_over_many_seeds() {
        let seeds: Vec<u64> = (0..200).collect();
        let result = run_batch(
            RuleSet::default(),
            players(&["buy_all", "buy_none"]),
            &seeds,
        )
        .unwrap();
        assert_eq!(result.aggregate.games, 200);
        let buy_all_rate = result.aggregate.win_rate_by_strategy["buy_all"];
        let buy_none_rate = result.aggregate.win_rate_by_strategy["buy_none"];
        assert!(
            buy_all_rate > buy_none_rate,
            "buy_all ({buy_all_rate}) should beat buy_none ({buy_none_rate})"
        );
        let cell = result.aggregate.head_to_head["buy_all"]["buy_none"];
        assert_eq!(cell.total, 200); // every 2-player game resolves the only pairing
        assert!(cell.wins > cell.total / 2);
    }

    #[test]
    fn an_unknown_strategy_fails_the_whole_batch_up_front() {
        let err = run_batch(
            RuleSet::default(),
            players(&["not_a_real_strategy", "buy_none"]),
            &[1, 2, 3],
        );
        assert!(matches!(err, Err(ConfigError::UnknownStrategy(_))));
    }

    #[test]
    fn same_base_seed_reproduces_the_same_batch_result() {
        let seeds = vec![7, 8, 9];
        let players = players(&["buy_good", "buy_bad"]);
        let a = run_batch(RuleSet::default(), players.clone(), &seeds).unwrap();
        let b = run_batch(RuleSet::default(), players, &seeds).unwrap();
        let summaries = |r: &BatchResult| {
            r.per_game
                .iter()
                .map(|g| (g.seed, g.winner, g.turns))
                .collect::<Vec<_>>()
        };
        assert_eq!(summaries(&a), summaries(&b));
    }

    #[test]
    fn distribution_summary_of_a_single_value_collapses_to_that_value() {
        let mut values = vec![1500.0];
        let summary = DistributionSummary::from_values(&mut values);
        assert_eq!(summary.mean, 1500.0);
        assert_eq!(summary.median, 1500.0);
        assert_eq!(summary.p10, 1500.0);
        assert_eq!(summary.p90, 1500.0);
    }
}
