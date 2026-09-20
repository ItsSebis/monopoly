# Roadmap

Seven phases, each a substantial, demoable increment rather than a small step. Later phases depend on earlier ones being solid — in particular, no UI work starts until the headless engine is already trustworthy (Phases 1–4), because the UI is meant to be a thin, swappable interface over a system that already works without it.

## Phase 1 — Core engine & headless MVP

Board data (40 spaces), `RuleSet`/`PlayerConfig` schema, seeded RNG, the basic turn loop: dice rolling including doubles and the 3-doubles-to-jail rule, movement with GO salary, a buy-or-skip decision on landing (no auctions yet), rent payment, income tax, and going to jail. Built-in strategies (Buy All/Good/Bad/None) implemented for purchase decisions only. Bankruptcy-to-bank ends a player's game when they can't cover a payment; the game ends when one player remains. No houses/hotels, no auctions, no mortgages, no full Chance/Community Chest decks yet (see [game-rules.md](./game-rules.md) — these are deliberately deferred to Phase 2). Rule-fidelity unit tests for everything in scope ([testing-and-validation.md](./testing-and-validation.md)).

**Demo**: `monopoly run` completes one full headless game and prints the winner plus its event log.

## Phase 2 — Full rules fidelity

Houses and hotels (with the even-build rule and the fixed 32/12 bank supply), mortgaging, the complete Chance and Community Chest decks (all effects), auctions when a purchase is declined, full jail decision logic (pay/roll/use card, including the 3-turn forced-exit cap), and the configurable free-parking-pot and income-tax modes. Strategies extended to cover building, mortgaging, and jail decisions, not just purchasing. Rule-fidelity tests extended to match, plus the first scenario/replay regression tests.

**Demo**: a full game under a chosen `RuleSet` that matches real Monopoly rules end to end, including at least one bankruptcy-to-player transfer and one completed monopoly with houses built.

## Phase 3 — Batch simulation & statistics engine

The parallel batch runner (`rayon`, one game per seed), the full metrics/aggregation layer from [analysis-and-metrics.md](./analysis-and-metrics.md), and the `monopoly batch` CLI subcommand with JSON/CSV export. Property-based invariant tests are introduced here, now that there's real rule interaction to stress-test. Still entirely headless.

**Demo**: run 10,000 games across a mix of the four built-in strategies and export win-rate, ROI, and the strategy head-to-head matrix.

## Phase 4 — Persistence & archive server

The `server` crate (axum + SQLite): `POST /runs`, `POST /runs/batch`, `GET /runs`, `GET /runs/{id}`, `GET /runs/{id}/games/{seed}`, `DELETE /runs/{id}` ([api.md](./api.md)), storing batch runs as config+seeds (not full logs) per the [determinism-as-storage-optimization](./architecture.md#determinism-as-a-storage-optimization) decision. The CLI's `--archive-url` flag is wired up.

**Demo**: run a headless batch, archive it to a running server, then fetch it back — including regenerating one specific game's full event log from just its seed via `GET /runs/{id}/games/{seed}`.

## Phase 5 — Browser UI: board & live playback

The `web/` Vite/TypeScript project: board renderer, `RuleSet`/`PlayerConfig` config forms, `engine-wasm` running inside a Web Worker, and playback controls (speed multiplier, pause, step) for a single live game ([frontend.md](./frontend.md)). No batch or history features yet — purely "configure and watch one game."

**Demo**: configure a game entirely in the browser and watch it play out on the board at an adjustable speed, with zero server running.

## Phase 6 — Analysis dashboard & history browser

Charts (Chart.js) for every metric in [analysis-and-metrics.md](./analysis-and-metrics.md), the browser's batch-run flow (`POST /runs/batch`, aggregate dashboard), and the history browser (server-backed list/detail views with a `localStorage` recent-runs cache, degrading gracefully with no server reachable). See [frontend.md](./frontend.md#batch-runs-from-the-ui) for why a real progress bar isn't part of this: a 5,000-game batch finishes server-side in 0.17s, before a poll could land.

**Demo**: launch a 5,000-game batch from the browser (done in well under a second), then browse the resulting charts and drill into one specific game's full replay from the aggregate view.

## Phase 7 — Advanced strategies (stretch)

Player-initiated trading (`Strategy::decide_trade`, added to the trait alongside the existing decision hooks), a strategy-tournament mode that runs every registered strategy against every other pairing automatically and reports the full head-to-head matrix, and new strategies built on the sourced gaps identified in [player-strategies.md#where-the-built-in-strategies-diverge-from-this](./player-strategies.md#where-the-built-in-strategies-diverge-from-this): a landing-frequency-weighted purchase/bid heuristic (valuing Orange/Red-style high-traffic properties above their raw rent-to-price ratio), a phase-dependent jail policy keyed on opponents' hotel risk (leave fast early, stay once opponents hold built monopolies — not just once the acting player holds one), house-supply-denial building (deliberately locking up the fixed 32-house stock to block opponents' expensive groups), and auction denial bidding (bidding to keep a property from an opponent independent of the acting strategy's own interest in it).

**Demo**: a trading-capable strategy shows a measurably higher win rate than the same base strategy without trading, across a large batch — evidence the addition actually changed outcomes rather than just adding surface area.
