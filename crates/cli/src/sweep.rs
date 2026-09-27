//! `monopoly sweep` (Phase 8, docs/roadmap.md): an empirical search over
//! `Configurable`'s axis space (`monopoly_engine::strategies::configurable`)
//! for the strongest configuration against the 5 named built-in strategies,
//! run across 4 ruleset environments (`examples/sweep_*.toml`) so the answer
//! isn't an artifact of one particular rule mix. See
//! docs/strategy-search-results.md for the actual results this produced and
//! `docs/roadmap.md`'s Phase 8 entry for the summary.
//!
//! This is a one-off research tool, not a permanent feature: it favors
//! straightforward, readable code over generality.

use std::fs;
use std::path::Path;

use monopoly_engine::{
    derive_batch_seeds, run_batch, AuctionPolicy, BuildPolicy, ConfigurableParams, JailPolicy,
    PlayerConfig, RuleSet, TradePolicy, Valuation,
};

use crate::load_config;

/// The 5 named built-in strategies a sweep candidate is measured against,
/// one seat each, every sweep run.
const PANEL: [&str; 5] = ["buy_all", "buy_good", "buy_bad", "buy_none", "buy_shrewd"];

/// Reserve values the top stage-1 combo (and the overall winner, pooled
/// across every ruleset) is refined over.
const REFINE_RESERVES: [i64; 5] = [50, 100, 150, 200, 300];

const SWEEP_RULESETS: [&str; 4] = [
    "sweep_baseline.toml",
    "sweep_trading.toml",
    "sweep_house_rules.toml",
    "sweep_maximal.toml",
];

/// One evaluated `ConfigurableParams`'s pooled result within a single
/// ruleset (across all 6 seat rotations).
#[derive(Debug, Clone)]
struct SweepRow {
    params: ConfigurableParams,
    win_rate: f64,
    games: usize,
}

/// Everything kept from sweeping one ruleset: enough to write its CSV and
/// to compute the cross-ruleset mean rank afterward.
struct RulesetOutcome {
    /// The ruleset file's stem, e.g. `"sweep_baseline"` — used for the CSV
    /// filename and the summary table.
    name: String,
    /// The 24 stage-1 combos, in the fixed order `stage_one_combos()`
    /// produces them (so index `i` means the same axis combo in every
    /// ruleset) — used for the mean-rank computation.
    stage1: Vec<SweepRow>,
    /// `stage1` plus the reserve/valuation refinement of this ruleset's own
    /// top stage-1 combo, sorted by win rate descending — written as this
    /// ruleset's CSV.
    ranked: Vec<SweepRow>,
}

pub fn sweep(
    rules_dir: &Path,
    games_per_matchup: u32,
    seed: u64,
    out_dir: &Path,
) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("creating {}: {e}", out_dir.display()))?;

    let mut rulesets = Vec::with_capacity(SWEEP_RULESETS.len());
    for file_name in SWEEP_RULESETS {
        let path = rules_dir.join(file_name);
        let rules: RuleSet = load_config(&path)?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file_name)
            .to_string();
        rulesets.push((name, rules));
    }

    let mut outcomes = Vec::with_capacity(rulesets.len());
    for (name, rules) in &rulesets {
        println!("sweeping {name}...");
        let outcome = sweep_one_ruleset(name, rules, games_per_matchup, seed);

        let csv_path = out_dir.join(format!("{name}.csv"));
        write_ranked_csv(&csv_path, &outcome.ranked)?;
        println!(
            "  wrote {} rows to {} (best: {:.1}% win rate, {:?})",
            outcome.ranked.len(),
            csv_path.display(),
            outcome.ranked[0].win_rate * 100.0,
            outcome.ranked[0].params
        );

        outcomes.push(outcome);
    }

    let (winning_axis_combo, rank_table) = pick_overall_winner_by_mean_rank(&outcomes);

    // Refine the overall winning axis combo's reserve/valuation once more,
    // by mean rank *within each ruleset* rather than a single number pooled
    // across all 4 (see `refine_by_rank`'s doc comment for why: the 4
    // rulesets' completion rates differ by an order of magnitude, so a raw
    // pooled win rate would just be dominated by whichever ruleset happens
    // to produce winners most often, rather than reflecting genuine
    // relative strength) — the deciding comparison for what `BuyOptimal`
    // actually hardcodes.
    let (final_params, refine_table) =
        refine_by_rank(&winning_axis_combo, &rulesets, games_per_matchup, seed);

    let summary_path = out_dir.join("summary.md");
    write_summary(
        &summary_path,
        &outcomes,
        &rank_table,
        &final_params,
        &refine_table,
    )?;
    println!("wrote summary to {}", summary_path.display());
    println!("overall winner: {final_params:?}");

    Ok(())
}

/// Stage 1 (24 combos, ranked) plus the reserve/valuation refinement of this
/// ruleset's own top combo — see the module doc comment's step list.
fn sweep_one_ruleset(
    name: &str,
    rules: &RuleSet,
    games_per_matchup: u32,
    seed: u64,
) -> RulesetOutcome {
    let combos = stage_one_combos();
    let stage1: Vec<SweepRow> = combos
        .iter()
        .map(|params| score_combo(rules, params, seed, games_per_matchup))
        .collect();

    let mut stage1_sorted = stage1.clone();
    stage1_sorted.sort_by(|a, b| b.win_rate.total_cmp(&a.win_rate));
    let top_combo = stage1_sorted[0].params;

    let mut refine = Vec::with_capacity(REFINE_RESERVES.len() + 1);
    for &reserve in &REFINE_RESERVES {
        let params = ConfigurableParams {
            reserve,
            ..top_combo
        };
        refine.push(score_combo(rules, &params, seed, games_per_matchup));
    }
    let rent_to_price_variant = ConfigurableParams {
        valuation: Valuation::RentToPrice,
        ..top_combo
    };
    refine.push(score_combo(
        rules,
        &rent_to_price_variant,
        seed,
        games_per_matchup,
    ));

    let mut ranked: Vec<SweepRow> = stage1.iter().cloned().chain(refine).collect();
    ranked.sort_by(|a, b| b.win_rate.total_cmp(&a.win_rate));

    RulesetOutcome {
        name: name.to_string(),
        stage1,
        ranked,
    }
}

/// Every `JailPolicy` x `BuildPolicy` x `AuctionPolicy` x `TradePolicy`
/// combination (3x2x2x2 = 24), each fixed at `Valuation::Weighted,
/// reserve: 100` — the common candidate set scored identically in every
/// ruleset, and so the one the cross-ruleset mean rank is computed over.
fn stage_one_combos() -> Vec<ConfigurableParams> {
    let jails = [
        JailPolicy::PayIfAffordable,
        JailPolicy::Patient,
        JailPolicy::HotelRisk,
    ];
    let builds = [
        BuildPolicy {
            stop_before_hotel: true,
        },
        BuildPolicy {
            stop_before_hotel: false,
        },
    ];
    let auctions = [
        AuctionPolicy::ValuationCapped,
        AuctionPolicy::ValuationCappedWithDenial,
    ];
    let trades = [TradePolicy::Never, TradePolicy::MonopolyCompleting];

    let mut combos = Vec::with_capacity(24);
    for &jail in &jails {
        for &build in &builds {
            for &auction in &auctions {
                for &trade in &trades {
                    combos.push(ConfigurableParams {
                        valuation: Valuation::Weighted,
                        jail,
                        build,
                        auction,
                        trade,
                        reserve: 100,
                    });
                }
            }
        }
    }
    combos
}

fn score_combo(
    rules: &RuleSet,
    params: &ConfigurableParams,
    seed: u64,
    games_per_matchup: u32,
) -> SweepRow {
    let (win_rate, games) = run_one_combo(rules, params, seed, games_per_matchup);
    SweepRow {
        params: *params,
        win_rate,
        games,
    }
}

/// Seats a `cfg:{json}` candidate for `params` in each of 6 rotations
/// alongside the fixed 5-strategy `PANEL` (the candidate occupies every seat
/// exactly once; the panel keeps its relative order around it),
/// `games_per_matchup / 6` games per rotation, and returns the pooled win
/// rate (and total games) across every rotation.
fn run_one_combo(
    rules: &RuleSet,
    params: &ConfigurableParams,
    seed: u64,
    games_per_matchup: u32,
) -> (f64, usize) {
    let candidate_id = format!(
        "cfg:{}",
        serde_json::to_string(params).expect("ConfigurableParams always serializes")
    );
    let games_per_rotation = (games_per_matchup as usize / PANEL.len().saturating_add(1)).max(1);

    let mut total_wins = 0usize;
    let mut total_games = 0usize;
    for rotation in 0..=PANEL.len() {
        let mut players: Vec<PlayerConfig> = PANEL
            .iter()
            .map(|&s| PlayerConfig {
                name: s.to_string(),
                strategy: s.to_string(),
            })
            .collect();
        players.insert(
            rotation,
            PlayerConfig {
                name: "candidate".to_string(),
                strategy: candidate_id.clone(),
            },
        );

        // Simplicity over cleverness (see module doc comment): just offset
        // the base seed per rotation rather than threading a rotation
        // parameter through `derive_batch_seeds`.
        let seeds = derive_batch_seeds(seed + rotation as u64 * 10_000, games_per_rotation);
        let result = run_batch(rules.clone(), players, &seeds, None)
            .expect("the candidate's cfg: id and every panel id are always valid");
        total_wins += result
            .per_game
            .iter()
            .filter(|g| g.winner == Some(rotation))
            .count();
        total_games += result.per_game.len();
    }
    (total_wins as f64 / total_games.max(1) as f64, total_games)
}

/// One row of the cross-ruleset mean-rank table: `combo`'s 1-based rank
/// within each ruleset's own stage-1 (24-combo) ranking, and the mean of
/// those 4 ranks.
struct RankRow {
    combo: ConfigurableParams,
    ranks: Vec<(String, usize, f64)>, // (ruleset name, rank, win_rate)
    mean_rank: f64,
}

/// Ranks the 24 stage-1 combos within each ruleset (1 = best), then picks
/// the combo with the best (lowest) mean rank across all 4 rulesets — tied
/// broken by mean win rate — as the overall winner. Returns that combo
/// alongside the full rank table (for the summary doc).
fn pick_overall_winner_by_mean_rank(
    outcomes: &[RulesetOutcome],
) -> (ConfigurableParams, Vec<RankRow>) {
    let combos = stage_one_combos();
    let mut table = Vec::with_capacity(combos.len());

    for combo in &combos {
        let mut ranks = Vec::with_capacity(outcomes.len());
        for outcome in outcomes {
            let mut sorted = outcome.stage1.clone();
            sorted.sort_by(|a, b| b.win_rate.total_cmp(&a.win_rate));
            let (rank, row) = sorted
                .iter()
                .enumerate()
                .find(|(_, row)| row.params == *combo)
                .expect("every stage-1 combo appears in every ruleset's stage1 list");
            ranks.push((outcome.name.clone(), rank + 1, row.win_rate));
        }
        let mean_rank = ranks.iter().map(|&(_, r, _)| r as f64).sum::<f64>() / ranks.len() as f64;
        table.push(RankRow {
            combo: *combo,
            ranks,
            mean_rank,
        });
    }

    table.sort_by(|a, b| {
        a.mean_rank
            .total_cmp(&b.mean_rank)
            .then_with(|| mean_win_rate(b).total_cmp(&mean_win_rate(a)))
    });
    let winner = table[0].combo;
    (winner, table)
}

fn mean_win_rate(row: &RankRow) -> f64 {
    row.ranks.iter().map(|&(_, _, wr)| wr).sum::<f64>() / row.ranks.len() as f64
}

/// One reserve/valuation candidate's result in the final refinement pass:
/// its win rate in every ruleset, plus its rank-based summary across them.
struct RefineCandidate {
    params: ConfigurableParams,
    /// (ruleset name, win rate, games) — one entry per ruleset.
    per_ruleset: Vec<(String, f64, usize)>,
    mean_rank: f64,
    mean_win_rate: f64,
}

/// Refines `combo`'s reserve (the same `REFINE_RESERVES` sweep as
/// `sweep_one_ruleset`) plus a `Valuation::RentToPrice` variant, one last
/// time — but by *rank within each ruleset*, not a single win rate pooled
/// across all 4. The 4 rulesets' completion rates span nearly an order of
/// magnitude (trading roughly triples how often any game produces a winner
/// at all — see docs/player-strategies.md's Trading section and
/// docs/roadmap.md's Phase 7 entry), so pooling raw win/loss counts across
/// them would let whichever ruleset happens to finish games most often
/// dominate the comparison; ranking within each ruleset first and averaging
/// the ranks treats all 4 environments as equally informative regardless of
/// how often they resolve. Returns the winning `ConfigurableParams` (what
/// `BuyOptimal` hardcodes) and the full candidate list for the summary.
fn refine_by_rank(
    combo: &ConfigurableParams,
    rulesets: &[(String, RuleSet)],
    games_per_matchup: u32,
    seed: u64,
) -> (ConfigurableParams, Vec<RefineCandidate>) {
    let mut candidates: Vec<ConfigurableParams> = REFINE_RESERVES
        .iter()
        .map(|&reserve| ConfigurableParams { reserve, ..*combo })
        .collect();
    candidates.push(ConfigurableParams {
        valuation: Valuation::RentToPrice,
        ..*combo
    });

    // results[i][j] = (win_rate, games) for candidate i in ruleset j.
    let results: Vec<Vec<(f64, usize)>> = candidates
        .iter()
        .map(|params| {
            rulesets
                .iter()
                .map(|(_, rules)| run_one_combo(rules, params, seed, games_per_matchup))
                .collect()
        })
        .collect();

    let mut ranked = Vec::with_capacity(candidates.len());
    for (i, &params) in candidates.iter().enumerate() {
        let mut per_ruleset = Vec::with_capacity(rulesets.len());
        let mut ranks = Vec::with_capacity(rulesets.len());
        for (j, (name, _)) in rulesets.iter().enumerate() {
            let (win_rate, games) = results[i][j];
            per_ruleset.push((name.clone(), win_rate, games));
            // 1-based rank among this refinement's own candidates, within
            // this one ruleset; ties (same win rate) share a rank.
            let rank = 1
                + (0..candidates.len())
                    .filter(|&k| results[k][j].0 > win_rate)
                    .count();
            ranks.push(rank);
        }
        let mean_rank = ranks.iter().sum::<usize>() as f64 / ranks.len() as f64;
        let mean_win_rate =
            per_ruleset.iter().map(|&(_, wr, _)| wr).sum::<f64>() / per_ruleset.len() as f64;
        ranked.push(RefineCandidate {
            params,
            per_ruleset,
            mean_rank,
            mean_win_rate,
        });
    }

    ranked.sort_by(|a, b| {
        a.mean_rank
            .total_cmp(&b.mean_rank)
            .then_with(|| b.mean_win_rate.total_cmp(&a.mean_win_rate))
    });
    let winner = ranked[0].params;
    (winner, ranked)
}

/// Minimal hand-rolled CSV writer, matching `main.rs`'s existing
/// `write_csv` style — not worth a dependency for nine columns.
fn write_ranked_csv(path: &Path, ranked: &[SweepRow]) -> Result<(), String> {
    let mut out = String::from(
        "combo_id,valuation,jail,stop_before_hotel,auction,trade,reserve,win_rate,games\n",
    );
    for (i, row) in ranked.iter().enumerate() {
        out.push_str(&format!(
            "{},{:?},{:?},{},{:?},{:?},{},{:.4},{}\n",
            i + 1,
            row.params.valuation,
            row.params.jail,
            row.params.build.stop_before_hotel,
            row.params.auction,
            row.params.trade,
            row.params.reserve,
            row.win_rate,
            row.games,
        ));
    }
    fs::write(path, out).map_err(|e| format!("writing {}: {e}", path.display()))
}

fn write_summary(
    path: &Path,
    outcomes: &[RulesetOutcome],
    rank_table: &[RankRow],
    final_params: &ConfigurableParams,
    refine_table: &[RefineCandidate],
) -> Result<(), String> {
    let mut out = String::new();
    out.push_str("# Phase 8 sweep summary\n\n");
    out.push_str(&format!(
        "Overall winner (best mean rank across all 4 rulesets' stage-1 rankings, \
         then refined by mean rank — within each ruleset, not pooled — over \
         reserve/valuation variants):\n\n```\n{final_params:?}\n```\n\n"
    ));

    out.push_str("## Per-ruleset best (stage-1 + local refinement)\n\n");
    out.push_str("| ruleset | best win rate | best config |\n|---|---|---|\n");
    for outcome in outcomes {
        out.push_str(&format!(
            "| {} | {:.1}% | `{:?}` |\n",
            outcome.name,
            outcome.ranked[0].win_rate * 100.0,
            outcome.ranked[0].params
        ));
    }

    out.push_str("\n## Cross-ruleset mean rank (top 5 of 24 stage-1 combos)\n\n");
    out.push_str("| mean rank | combo | ");
    for outcome in outcomes {
        out.push_str(&format!("{} rank (win rate) | ", outcome.name));
    }
    out.push_str("\n|---|---|");
    for _ in outcomes {
        out.push_str("---|");
    }
    out.push('\n');
    for row in rank_table.iter().take(5) {
        out.push_str(&format!("| {:.2} | `{:?}` | ", row.mean_rank, row.combo));
        for &(_, rank, wr) in &row.ranks {
            out.push_str(&format!("{rank} ({:.1}%) | ", wr * 100.0));
        }
        out.push('\n');
    }

    out.push_str(
        "\n## Reserve/valuation refinement of the overall winner (ranked within each ruleset)\n\n",
    );
    out.push_str("| config | mean rank | mean win rate | ");
    for outcome in outcomes {
        out.push_str(&format!("{} win rate | ", outcome.name));
    }
    out.push_str("\n|---|---|---|");
    for _ in outcomes {
        out.push_str("---|");
    }
    out.push('\n');
    for row in refine_table {
        out.push_str(&format!(
            "| reserve={}, valuation={:?} | {:.2} | {:.1}% | ",
            row.params.reserve,
            row.params.valuation,
            row.mean_rank,
            row.mean_win_rate * 100.0
        ));
        for &(_, wr, _) in &row.per_ruleset {
            out.push_str(&format!("{:.1}% | ", wr * 100.0));
        }
        out.push('\n');
    }

    fs::write(path, out).map_err(|e| format!("writing {}: {e}", path.display()))
}
