use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use monopoly_engine::{Game, GameConfig, GameResult, PlayerConfig};

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
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Run { config, seed, out } => run(&config, seed, out.as_deref()),
    }
}

fn run(config_path: &Path, seed: Option<u64>, out: Option<&Path>) -> ExitCode {
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };

    let seed = seed.unwrap_or_else(rand::random);
    let mut game = match Game::new(config.rules, &config.players, seed) {
        Ok(g) => g,
        Err(e) => return fail(&e.to_string()),
    };
    let result = game.run_to_completion();

    match out {
        Some(path) => {
            let output = OutputFile { seed, result: &result };
            let json = serde_json::to_string_pretty(&output).expect("GameResult is always serializable");
            if let Err(e) = fs::write(path, json) {
                return fail(&format!("writing {}: {e}", path.display()));
            }
            println!("seed {seed}: wrote {} events to {}", result.events.len(), path.display());
        }
        None => print_summary(seed, &config.players, &result),
    }

    ExitCode::SUCCESS
}

fn fail(message: &str) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::FAILURE
}

#[derive(serde::Serialize)]
struct OutputFile<'a> {
    seed: u64,
    #[serde(flatten)]
    result: &'a GameResult,
}

fn load_config(path: &Path) -> Result<GameConfig, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    if path.extension().is_some_and(|ext| ext == "json") {
        serde_json::from_str(&text).map_err(|e| format!("parsing {} as JSON: {e}", path.display()))
    } else {
        toml::from_str(&text).map_err(|e| format!("parsing {} as TOML: {e}", path.display()))
    }
}

fn print_summary(seed: u64, players: &[PlayerConfig], result: &GameResult) {
    println!("seed: {seed}");
    println!("turns: {}", result.turns);
    match result.winner {
        Some(w) => println!("winner: {} ({})", players[w].name, players[w].strategy),
        None => println!("winner: none (hit the internal safety cap without a sole survivor)"),
    }
    println!("events: {}", result.events.len());
    for (i, player) in result.final_state.players.iter().enumerate() {
        let properties = result.final_state.owners.iter().filter(|o| **o == Some(i)).count();
        println!("  {}: cash={} bankrupt={} properties={properties}", player.name, player.cash, player.bankrupt);
    }
}
