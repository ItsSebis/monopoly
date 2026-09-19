# Architecture

## Goals driving the shape of the system

- **One rules implementation.** The engine is written once, in Rust, and consumed unchanged by the CLI, the server, and the browser (via WASM). There is no second, browser-side reimplementation of the rules to keep in sync.
- **Headless always works.** The CLI never depends on the server or the browser. It links `engine` directly and runs entirely standalone.
- **Small surface area.** Four Rust crates and one small frontend project. No microservices, no message queue, no separate database service — SQLite is a file.

## Workspace layout

```
monopoly/
  Cargo.toml                # workspace manifest
  crates/
    engine/                 # pure simulation logic — no I/O, no async, no dependencies on cli/server/wasm
    engine-wasm/             # wasm-bindgen bindings around engine, built for the browser
    cli/                     # binary: headless `run` (single game) and `batch` (N games) subcommands
    server/                  # binary: axum HTTP API + SQLite archive
  web/                       # TypeScript + Vite frontend (separate npm project, depends on engine-wasm's build output)
  docs/                      # this documentation
```

`engine` is the only crate that knows Monopoly rules. Everything else is a thin shell around it:

- `cli` = engine + argument parsing + file/stdout I/O.
- `server` = engine + axum routes + SQLite persistence.
- `engine-wasm` = engine + `wasm-bindgen` glue exposing a JS-friendly API (create game, step one turn, get state as JSON).
- `web` = UI only. It never implements game rules — it calls into `engine-wasm` for the live game and the `server` API for everything else (batch runs, history).

## Data flow

```
                     ┌──────────────┐
                     │   engine     │  (rules, state, strategies, RNG, event log)
                     └──────┬───────┘
             ┌──────────────┼───────────────┬───────────────┐
             │              │               │               │
      ┌──────▼─────┐  ┌─────▼──────┐  ┌─────▼──────┐        │
      │    cli     │  │   server   │  │engine-wasm │        │
      │ (native)   │  │  (native)  │  │  (WASM)    │        │
      └──────┬─────┘  └─────┬──────┘  └─────┬──────┘        │
             │  optional     │ REST API      │ imported by   │
             │  archive push │               │ a Web Worker  │
             │        ┌──────▼──────┐  ┌─────▼──────┐        │
             └───────►│   SQLite    │  │    web     │◄───────┘
                      │  (archive)  │  │ (TS + Vite)│  fetches history via
                      └─────────────┘  └────────────┘  server REST API
```

## Native vs. WASM: two execution paths, one engine

There are two fundamentally different jobs the engine does, and they run through different paths:

1. **Live playback** — one game, animated turn-by-turn on the board, at a speed the user controls. This runs **in the browser**, via `engine-wasm`, inside a Web Worker (so board rendering never blocks on simulation). Running it client-side means the user can watch a game with zero server round-trips, and it works even with no server running at all.
2. **Batch statistics** — hundreds to tens of thousands of games run back-to-back with no animation, to produce aggregate stats. This **always runs natively** — either the CLI directly, or the server dispatching to native `engine` code — never through WASM. Native Rust routinely outperforms WASM by a meaningful margin on tight, branchy CPU loops like a Monopoly turn loop, and batch mode is precisely the case where that throughput matters (Phase 3 targets tens of thousands of games in a runtime of seconds, not minutes).

The browser never runs a batch itself; when the UI wants a batch run, it asks the server to run it natively and polls/fetches the result (see [api.md](./api.md)).

## Determinism as a storage optimization

Every game is a pure function of `(RuleSet config, RNG seed)`: same config, same seed, same game, byte-for-byte, forever (see [simulation-engine.md](./simulation-engine.md#determinism) for how the RNG is threaded through). This has a direct architectural consequence for the archive:

- For a **batch** of N games, the archive only needs to store the `RuleSet` and the N seeds used, plus whatever summary statistics were computed (win rates, net-worth curves, etc.) — not N full event logs. Any individual game from that batch can be regenerated on demand by re-running the engine with its exact `(config, seed)`, at native speed, in well under a second.
- For a **single live run** the user explicitly watches or saves, the full event log **is** stored, because it was already generated for playback and re-deriving it isn't necessary.

This keeps the archive small regardless of how many games have ever been simulated, without giving up the ability to inspect any individual game's full turn-by-turn detail later. See [data-model.md](./data-model.md#run-record) for the exact `Run` record shape this produces.

## Why this stays "small codebase"

- One language (Rust) for every piece of logic and every server; the only other language is the UI's TypeScript, which contains zero game rules.
- One `Strategy` trait, implemented by both built-in and custom strategies, used identically whether the engine is running headless, in a batch, or live in the browser.
- One event log format, produced by every run, consumed by playback (web), by statistics (analysis-and-metrics), and by the archive (server) — no per-consumer serialization formats.
- No database server, no message broker: SQLite file + a single axum binary.
