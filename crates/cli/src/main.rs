use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use monopoly_engine::{
    build_batch_run_record, compute_stats, derive_batch_seeds, BatchRunRecord, Board, Game,
    GameConfig, GameResult, PerGameStats, PlayerConfig, SingleRunRecord,
};
use serde::Serialize;

/// A live per-game progress bar to stderr when it's a real terminal; a
/// no-op, non-drawing bar otherwise (piped/redirected output, or CI) so
/// `--out`/`--csv` usage and scripted invocations stay clean.
fn progress_bar(games: usize) -> ProgressBar {
    if !std::io::stderr().is_terminal() {
        return ProgressBar::hidden();
    }
    let bar = ProgressBar::new(games as u64);
    bar.set_style(
        ProgressStyle::with_template("{bar:40} {pos}/{len} games ({eta} left)")
            .expect("valid progress bar template"),
    );
    bar
}

#[derive(Parser)]
#[command(name = "monopoly", about = "Headless Monopoly simulator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a single game to completion.
    Run {
        /// Path to a TOML or JSON config file (RuleSet + players).
        #[arg(long)]
        config: PathBuf,
        /// RNG seed; a random one is generated (and printed) if omitted.
        #[arg(long)]
        seed: Option<u64>,
        /// Write the full result as JSON here instead of printing a summary.
        #[arg(long)]
        out: Option<PathBuf>,
        /// POST the run to a running `monopoly-server`'s `/runs` endpoint
        /// (e.g. `http://localhost:3000`) after the local `--out`/summary
        /// handling completes.
        #[arg(long)]
        archive_url: Option<String>,
    },
    /// Run many games in parallel and report aggregate statistics.
    Batch {
        /// Path to a TOML or JSON config file (RuleSet + players).
        #[arg(long)]
        config: PathBuf,
        /// Number of games to run.
        #[arg(long)]
        games: usize,
        /// Base seed the per-game seeds are deterministically derived from;
        /// a random one is generated (and printed) if omitted.
        #[arg(long)]
        seed: Option<u64>,
        /// Write the full batch result (per-game summaries + aggregate
        /// statistics) as JSON here instead of printing a summary.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Write one row per game (seed, winner, turns) here as CSV.
        #[arg(long)]
        csv: Option<PathBuf>,
        /// POST the batch to a running `monopoly-server`'s `/runs` endpoint
        /// (e.g. `http://localhost:3000`) after the local `--out`/`--csv`/
        /// summary handling completes.
        #[arg(long)]
        archive_url: Option<String>,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Run {
            config,
            seed,
            out,
            archive_url,
        } => run(&config, seed, out.as_deref(), archive_url.as_deref()),
        Command::Batch {
            config,
            games,
            seed,
            out,
            csv,
            archive_url,
        } => batch(
            &config,
            games,
            seed,
            out.as_deref(),
            csv.as_deref(),
            archive_url.as_deref(),
        ),
    }
}

fn run(
    config_path: &Path,
    seed: Option<u64>,
    out: Option<&Path>,
    archive_url: Option<&str>,
) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };

    let seed = seed.unwrap_or_else(rand::random);
    let mut game = match Game::new(config.rules.clone(), &config.players, seed) {
        Ok(g) => g,
        Err(e) => return fail(&e.to_string()),
    };
    let result = game.run_to_completion();
    let board = Board::standard();
    let final_stats = compute_stats(&board, &config.players, &config.rules, &result);

    if out.is_none() {
        print_summary(seed, &config.players, &result, &final_stats);
    }

    if out.is_some() || archive_url.is_some() {
        // Built by hand rather than via `build_single_run_record` — that
        // helper re-simulates the game itself, which would run it twice.
        // `final_state` is deliberately not part of the archived shape
        // (docs/data-model.md): it's fully derivable by replaying `events`,
        // the same determinism-as-storage-optimization principle
        // `docs/architecture.md` already applies to batches.
        let record = SingleRunRecord {
            rule_set: config.rules,
            players: config.players,
            seed,
            events: result.events,
            final_stats,
        };

        if let Some(path) = out {
            if let Err(e) = write_json(path, &record) {
                return fail(&e);
            }
            println!(
                "seed {seed}: wrote {} events to {}",
                record.events.len(),
                path.display()
            );
        }
        if let Some(url) = archive_url {
            if let Err(e) = archive(url, &record) {
                return fail(&e);
            }
        }
    }

    ExitCode::SUCCESS
}

fn batch(
    config_path: &Path,
    games: usize,
    seed: Option<u64>,
    out: Option<&Path>,
    csv: Option<&Path>,
    archive_url: Option<&str>,
) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };

    let base_seed = seed.unwrap_or_else(rand::random);
    let seeds = derive_batch_seeds(base_seed, games);

    let bar = progress_bar(games);
    let record = build_batch_run_record(config.rules, config.players, seeds, Some(&|| bar.inc(1)));
    bar.finish_and_clear();
    let record = match record {
        Ok(r) => r,
        Err(e) => return fail(&e.to_string()),
    };

    if let Some(path) = csv {
        if let Err(e) = write_csv(path, &record) {
            return fail(&e);
        }
        println!(
            "wrote {} rows to {}",
            record.per_game_summary.len(),
            path.display()
        );
    }

    match out {
        Some(path) => {
            if let Err(e) = write_json(path, &record) {
                return fail(&e);
            }
            println!(
                "base seed {base_seed}: wrote {} games to {}",
                record.per_game_summary.len(),
                path.display()
            );
        }
        None => print_batch_summary(base_seed, &record),
    }

    if let Some(url) = archive_url {
        if let Err(e) = archive(url, &record) {
            return fail(&e);
        }
    }

    ExitCode::SUCCESS
}

fn fail(message: &str) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::FAILURE
}

fn load_config(path: &Path) -> Result<GameConfig, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    if path.extension().is_some_and(|ext| ext == "json") {
        serde_json::from_str(&text).map_err(|e| format!("parsing {} as JSON: {e}", path.display()))
    } else {
        toml::from_str(&text).map_err(|e| format!("parsing {} as TOML: {e}", path.display()))
    }
}

fn write_json(path: &Path, output: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(output).expect("simulation output is serializable");
    fs::write(path, json).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// POSTs a `SingleRunRecord`/`BatchRunRecord` to `{archive_url}/runs`
/// (`docs/api.md`) and prints the archived id. `ureq` is a small, blocking
/// HTTP client — the CLI stays fully synchronous, with no reason to pull
/// `tokio` in just for this one request.
fn archive(archive_url: &str, record: &impl Serialize) -> Result<(), String> {
    let url = format!("{}/runs", archive_url.trim_end_matches('/'));
    let response = ureq::post(&url)
        .send_json(record)
        .map_err(|e| format!("archiving to {url}: {e}"))?;
    let body: serde_json::Value = response
        .into_json()
        .map_err(|e| format!("reading archive response from {url}: {e}"))?;
    match body.get("id").and_then(|id| id.as_str()) {
        Some(id) => println!("archived as {id} ({url})"),
        None => {
            return Err(format!(
                "archive response from {url} had no `id` field: {body}"
            ))
        }
    }
    Ok(())
}

fn print_summary(seed: u64, players: &[PlayerConfig], result: &GameResult, stats: &PerGameStats) {
    println!("seed: {seed}");
    println!("turns: {}", result.turns);
    match result.winner {
        Some(w) => println!("winner: {} ({})", players[w].name, players[w].strategy),
        None => println!("winner: none (hit the internal safety cap without a sole survivor)"),
    }
    println!("events: {}", result.events.len());
    println!("monopolies completed: {}", stats.monopolies_completed.len());
    for (i, player) in result.final_state.players.iter().enumerate() {
        let owned = result
            .final_state
            .properties
            .iter()
            .filter(|p| p.owner == Some(i));
        let properties = owned.clone().count();
        let houses = owned
            .clone()
            .filter(|p| p.houses > 0 && p.houses < 5)
            .count();
        let hotels = owned.clone().filter(|p| p.houses == 5).count();
        let mortgaged = owned.filter(|p| p.mortgaged).count();
        println!(
            "  {}: cash={} bankrupt={} properties={properties} (houses={houses} hotels={hotels} mortgaged={mortgaged})",
            player.name, player.cash, player.bankrupt
        );
    }
}

fn print_batch_summary(base_seed: u64, record: &BatchRunRecord) {
    println!("base seed: {base_seed}");
    println!("games: {}", record.aggregate_stats.games);

    println!("win rate by strategy:");
    for (strategy, rate) in &record.aggregate_stats.win_rate_by_strategy {
        println!("  {strategy}: {:.1}%", rate * 100.0);
    }

    println!("roi by strategy (rent collected / cost basis):");
    for (strategy, roi) in &record.aggregate_stats.roi_by_strategy {
        println!("  {strategy}: {roi:.2}");
    }

    println!("head-to-head win rate (row beat column, when one of the two won):");
    for (winner, opponents) in &record.aggregate_stats.head_to_head {
        for (loser, cell) in opponents {
            if cell.total > 0 {
                println!(
                    "  {winner} vs {loser}: {:.1}% ({}/{})",
                    100.0 * cell.wins as f64 / cell.total as f64,
                    cell.wins,
                    cell.total
                );
            }
        }
    }
}

/// Minimal hand-rolled CSV writer (one flat summary row per game) — not
/// worth a dependency for four columns; `name`/`strategy` fields are quoted
/// since they come from user-supplied config.
fn write_csv(path: &Path, record: &BatchRunRecord) -> Result<(), String> {
    let mut out = String::from("seed,winner_index,winner_name,winner_strategy,turns\n");
    for game in &record.per_game_summary {
        let (index, name, strategy) = match game.winner {
            Some(w) => (
                w.to_string(),
                csv_field(&record.players[w].name),
                csv_field(&record.players[w].strategy),
            ),
            None => (String::new(), String::new(), String::new()),
        };
        out.push_str(&format!(
            "{},{index},{name},{strategy},{}\n",
            game.seed, game.turns
        ));
    }
    fs::write(path, out).map_err(|e| format!("writing {}: {e}", path.display()))
}

fn csv_field(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
