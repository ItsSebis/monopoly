# Headless CLI

The `cli` crate is the reference proof that the browser is optional: everything it does is a thin wrapper over `engine`, with no dependency on `server` or `web`. It's also the fastest way to iterate on the engine during development.

## `monopoly run` — single game

```sh
monopoly run --config game.toml --seed 1234 --out result.json
```

- `--config` — a TOML or JSON file matching `RuleSet` + `players` ([data-model.md](./data-model.md)). As of Phase 3 there's no per-field flag override (e.g. no `--starting-cash`) — edit the file for a different `RuleSet`.
- `--seed` — optional; if omitted, a random seed is generated and printed (so the run can be reproduced later).
- `--out` — write the full event log plus `final_stats` (the per-run metrics from [analysis-and-metrics.md](./analysis-and-metrics.md), computed by `stats::compute_stats`) as JSON to a file; if omitted, prints a short human-readable summary to stdout instead (winner, turns, event count, monopolies completed, and each player's final holdings).
- `--archive-url` (Phase 4, not yet implemented) — will additionally `POST` the result to a running server's `/runs` endpoint ([api.md](./api.md#post-runs)) after the local write/print completes.

## `monopoly batch` — many games

```sh
monopoly batch --config game.toml --games 10000 --seed 42 --out results.json --csv results.csv
```

- `--games` — how many games to simulate.
- `--seed` — the *base* seed; if omitted, a random one is generated and printed. Per-game seeds are drawn deterministically from a `StdRng` seeded with it, so the whole batch reproduces from that one number (see `same_base_seed_reproduces_the_same_batch_result` in `crates/engine/src/batch.rs`).
- `--out` — writes the full `BatchResult` (every game's `(seed, winner, turns)` plus `AggregateStats` — win rate and ROI by strategy, the head-to-head matrix, game-length/bankruptcy-turn/dice-roll/landing distributions, and final net worth by strategy) as JSON.
- `--csv` — writes one flat row per game (`seed,winner_index,winner_name,winner_strategy,turns`) for spreadsheet-style analysis; can be combined with `--out` in the same invocation.
- With neither `--out` nor `--csv`, prints win rate, ROI, and the head-to-head matrix to stdout — the numbers [roadmap.md](./roadmap.md)'s Phase 3 demo asks for.
- `--archive-url` (Phase 4, not yet implemented) — will push the batch record to a server.
- Runs in parallel across available cores via the engine's `rayon`-based batch runner ([simulation-engine.md](./simulation-engine.md#batch-execution)); no server or GPU involved. Each game's per-turn detail is folded into the aggregate and discarded immediately, so peak memory stays roughly constant as `--games` grows rather than scaling with total turns played (see `batch.rs`'s module doc comment).
- A batch with no `RuleSet.max_turns` set falls back to the engine's internal safety cap (20,000 turns) per game — for strategy mixes that don't reliably terminate without trading (see [game-rules.md](./game-rules.md#bankruptcy)), most of a large batch can be spent on games that never resolve. Setting `max_turns` in the config bounds this: a bounded 10,000-game batch typically finishes in well under a second, versus several seconds uncapped.

## Config file format

```toml
[rules]
starting_cash = 1500
go_salary = 200
income_tax_mode = "choice"
auction_on_decline = true
free_parking_pot = false
max_turns = 1000        # optional; recommended for batch runs — see above

[[players]]
name = "P1"
strategy = "buy_good"

[[players]]
name = "P2"
strategy = "buy_all"
```

This is the exact `RuleSet`/`PlayerConfig` shape from [data-model.md](./data-model.md), just in TOML instead of JSON — the CLI accepts either.

## Exit codes and errors

Config validation errors (unknown strategy id, fewer than two players) fail fast with a specific message before any simulation starts, rather than partway through a batch — `run_batch` validates the config once up front against a throwaway game before dispatching any seed to `rayon`.
