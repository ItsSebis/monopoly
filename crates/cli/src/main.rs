use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use monopoly_engine::{
    compute_stats, run_batch, BatchResult, Board, Game, GameConfig, GameResult, PerGameStats,
    PlayerConfig,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;

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
    },
}

#[derive(Serialize)]
struct RunOutputFile<'a> {
    seed: u64,
    #[serde(flatten)]
    result: &'a GameResult,
    final_stats: PerGameStats,
}

#[derive(Serialize)]
struct BatchOutputFile<'a> {
    base_seed: u64,
    #[serde(flatten)]
    result: &'a BatchResult,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Run { config, seed, out } => run(&config, seed, out.as_deref()),
        Command::Batch {
            config,
            games,
            seed,
            out,
            csv,
        } => batch(&config, games, seed, out.as_deref(), csv.as_deref()),
    }
}

fn run(config_path: &Path, seed: Option<u64>, out: Option<&Path>) -> ExitCode {
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

    match out {
        Some(path) => {
            let output = RunOutputFile {
                seed,
                result: &result,
                final_stats,
            };
            if let Err(e) = write_json(path, &output) {
                return fail(&e);
            }
            println!(
                "seed {seed}: wrote {} events to {}",
                result.events.len(),
                path.display()
            );
        }
        None => print_summary(seed, &config.players, &result, &final_stats),
    }

    ExitCode::SUCCESS
}

fn batch(
    config_path: &Path,
    games: usize,
    seed: Option<u64>,
    out: Option<&Path>,
    csv: Option<&Path>,
) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };

    let base_seed = seed.unwrap_or_else(rand::random);
    let mut rng = StdRng::seed_from_u64(base_seed);
    let seeds: Vec<u64> = (0..games).map(|_| rng.gen()).collect();

    let result = match run_batch(config.rules, config.players.clone(), &seeds) {
        Ok(r) => r,
        Err(e) => return fail(&e.to_string()),
    };

    if let Some(path) = csv {
        if let Err(e) = write_csv(path, &config.players, &result) {
            return fail(&e);
        }
        println!("wrote {} rows to {}", result.per_game.len(), path.display());
    }

    match out {
        Some(path) => {
            let output = BatchOutputFile {
                base_seed,
                result: &result,
            };
            if let Err(e) = write_json(path, &output) {
                return fail(&e);
            }
            println!(
                "base seed {base_seed}: wrote {} games to {}",
                result.per_game.len(),
                path.display()
            );
        }
        None => print_batch_summary(base_seed, &result),
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

fn print_batch_summary(base_seed: u64, result: &BatchResult) {
    println!("base seed: {base_seed}");
    println!("games: {}", result.aggregate.games);

    println!("win rate by strategy:");
    for (strategy, rate) in &result.aggregate.win_rate_by_strategy {
        println!("  {strategy}: {:.1}%", rate * 100.0);
    }

    println!("roi by strategy (rent collected / cost basis):");
    for (strategy, roi) in &result.aggregate.roi_by_strategy {
        println!("  {strategy}: {roi:.2}");
    }

    println!("head-to-head win rate (row beat column, when one of the two won):");
    for (winner, opponents) in &result.aggregate.head_to_head {
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
fn write_csv(path: &Path, players: &[PlayerConfig], result: &BatchResult) -> Result<(), String> {
    let mut out = String::from("seed,winner_index,winner_name,winner_strategy,turns\n");
    for game in &result.per_game {
        let (index, name, strategy) = match game.winner {
            Some(w) => (
                w.to_string(),
                csv_field(&players[w].name),
                csv_field(&players[w].strategy),
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
