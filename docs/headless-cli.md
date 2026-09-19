# Headless CLI

The `cli` crate is the reference proof that the browser is optional: everything it does is a thin wrapper over `engine`, with no dependency on `server` or `web`. It's also the fastest way to iterate on the engine during development.

## `monopoly run` — single game

```sh
monopoly run --config game.toml --seed 1234 --out result.json
```

- `--config` — a TOML or JSON file matching `RuleSet` + `players` ([data-model.md](./data-model.md)). Individual fields can be overridden with flags (e.g. `--starting-cash 2000`) for quick experiments without editing the file.
- `--seed` — optional; if omitted, a random seed is generated and printed (so the run can be reproduced later).
- `--out` — write the full event log + final stats (a `single` [Run record](./data-model.md#run-record)) as JSON to a file; if omitted, prints a human-readable turn-by-turn summary to stdout instead.
- `--archive-url` — optional; if set, also `POST`s the result to a running server's `/runs` endpoint ([api.md](./api.md#post-runs)) after the local write/print completes. Archiving is always additive, never required.

## `monopoly batch` — many games

```sh
monopoly batch --config game.toml --games 10000 --out results.csv
```

- `--games` — how many games to simulate. Seeds are generated from a base seed (`--seed`, optional) so the whole batch is reproducible from one number.
- `--out` — `.json` writes a full `batch` [Run record](./data-model.md#run-record) (seeds + per-game summaries + aggregate stats); `.csv` writes one row per game (seed, winner, turn count, and the flattened top-line stats) for quick spreadsheet/pandas-style analysis outside the tool entirely.
- `--archive-url` — same as `run`, pushes the batch record to a server if provided.
- Runs in parallel across available cores via the engine's `rayon`-based batch runner ([simulation-engine.md](./simulation-engine.md#batch-execution)); no server or GPU involved.

## Config file format

```toml
[rules]
starting_cash = 1500
go_salary = 200
income_tax_mode = "choice"
auction_on_decline = true
free_parking_pot = false

[[players]]
name = "P1"
strategy = "buy_good"

[[players]]
name = "P2"
strategy = "buy_all"
```

This is the exact `RuleSet`/`PlayerConfig` shape from [data-model.md](./data-model.md), just in TOML instead of JSON — the CLI accepts either.

## Exit codes and errors

Config validation errors (unknown strategy id, out-of-range values) fail fast with a specific message before any simulation starts, rather than partway through a batch — cheap to check up front since `RuleSet`/`PlayerConfig` validation doesn't require running the engine.
