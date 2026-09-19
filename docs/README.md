# Monopoly Simulator — Documentation

A Monopoly game simulator that plays out full games between configurable, automated players. Rules, player count, and per-player behavior are all configurable. A game can be watched live on a rendered board in the browser at adjustable speed, or run headlessly (no browser at all) in large batches purely to collect statistics on how different strategies and rule variants perform. Every run produces a detailed analysis: the winner, and as much collected data as possible, visualized as graphs.

Two constraints shape every decision in this project:

1. **Small codebase.** One simulation engine, written once, is reused by the headless CLI, the archive server, and the browser UI. No rules logic is duplicated across languages or layers.
2. **Headless-first.** The browser is an interface to the simulator, never a requirement for running one. Every capability (single game, batch of thousands, full analysis) works from the command line with no server and no browser.

## Stack at a glance

- **Rust workspace** (`engine`, `engine-wasm`, `cli`, `server` crates) — all simulation logic, the headless CLI, and the archive server. See [architecture.md](./architecture.md) for why.
- **TypeScript + Vite** (`web/`) — the browser UI only: board rendering, config forms, playback controls, charts, and a history browser. It contains no game rules of its own.

## Reading order

If you're new to this project, read in this order:

1. [architecture.md](./architecture.md) — system shape, crate boundaries, and the key design decisions (native vs. WASM, determinism-as-storage).
2. [game-rules.md](./game-rules.md) — the Monopoly rules being modeled and every configurable house-rule toggle.
3. [simulation-engine.md](./simulation-engine.md) — the turn state machine, event log, and the `Strategy` trait.
4. [player-strategies.md](./player-strategies.md) — the built-in automated behaviors (Buy All / Buy Good / Buy Bad / Buy None) and how to add custom ones.
5. [data-model.md](./data-model.md) — the shared schemas (`RuleSet`, `GameState`, events, archived runs) that every other doc references.
6. [api.md](./api.md) — the archive server's REST API.
7. [frontend.md](./frontend.md) — browser UI structure.
8. [headless-cli.md](./headless-cli.md) — running the simulator with no browser or server at all.
9. [analysis-and-metrics.md](./analysis-and-metrics.md) — every metric collected and how it's visualized.
10. [testing-and-validation.md](./testing-and-validation.md) — how rule-fidelity ("as close to the real game as possible") is verified.
11. [roadmap.md](./roadmap.md) — the phased build plan, from headless MVP to full browser analysis dashboard.
