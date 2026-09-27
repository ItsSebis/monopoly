# Roadmap

Eight phases, each a substantial, demoable increment rather than a small step. Later phases depend on earlier ones being solid — in particular, no UI work starts until the headless engine is already trustworthy (Phases 1–4), because the UI is meant to be a thin, swappable interface over a system that already works without it.

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

## Phase 7 — Advanced strategies

Player-initiated trading (`RuleSet.trading_enabled`, `Strategy::decide_trade`/`decide_trade_response` — two hooks, not the one originally sketched in [simulation-engine.md](./simulation-engine.md#the-strategy-trait), since a trade needs the counterparty's consent), a `monopoly tournament` CLI mode that runs every registered strategy against every other in one batch and reports the full head-to-head matrix, and a new `buy_shrewd` strategy built on the sourced gaps identified in [player-strategies.md#where-the-built-in-strategies-diverge-from-this](./player-strategies.md#where-the-built-in-strategies-diverge-from-this): a landing-frequency-weighted purchase/bid heuristic (valuing Orange/Red-style high-traffic properties above their raw rent-to-price ratio), a phase-dependent jail policy keyed on opponents' hotel risk (leave fast early, stay once opponents hold built monopolies — not just once the acting player holds one), house-supply-denial building (deliberately locking up the fixed 32-house stock to block opponents' expensive groups), and auction denial bidding (bidding to keep a property from an opponent independent of the acting strategy's own interest in it).

**Demo**: measured directly (`monopoly batch`, 3000 games, Buy Good/Buy All/Buy Bad, `max_turns: 300`, base seed 1) — the exact stalemate [player-strategies.md#trading](./player-strategies.md#trading)'s cited research predicts, and its fix: with `trading_enabled: false`, only 23.9% of games produce any winner within 300 turns; with it `true`, 70.5% do — and Buy Good's and Buy All's own win rates roughly triple and more than double respectively (11.2% → 36.0%, 12.7% → 34.4%) purely because far more games actually resolve. Evidence the addition changed outcomes, not just added surface area.

Also shipped this phase (not originally scoped here, added alongside on request): a stretched color scale for the landing-distribution heatmap, a CLI batch/tournament progress bar, animated board token movement, an English/German board-only language toggle, and two new house-rule toggles (`double_go_salary`, `unlimited_houses`) — see [game-rules.md](./game-rules.md), [frontend.md](./frontend.md), and [headless-cli.md](./headless-cli.md).

## Phase 8 — Empirical strategy search

A 2000-game tournament found Buy All (8.8% win rate) beating Buy Shrewd (5.9%) head-to-head (59.9%/40.1%) despite Buy Shrewd being the most behaviorally sophisticated built-in — reason enough to stop guessing at strategy design and search it empirically instead. Added `Configurable` (`crates/engine/src/strategies/configurable.rs`): a strategy expressed as data along 5 axes (valuation heuristic, jail policy, build-to-hotel-or-not, auction policy, trade policy, plus a cash reserve), runnable without a registered name via a new `cfg:{json}` strategy id (`make_strategy`). Several previously strategy-private helpers (`weighted_score`/`landing_weight`, `hotel_risk_jail_action`, and the denial-bid/valuation-capped-bid auction logic Buy Good/Buy Shrewd/`Configurable` all shared duplicated) and a new `affordable_jail_action` (factored out of Buy All) moved to `strategies/mod.rs` as shared helpers so every strategy builds on the same logic, with equivalence tests proving the refactor changed no existing strategy's behavior.

A new `monopoly sweep` CLI subcommand ([headless-cli.md](./headless-cli.md#monopoly-sweep--search-configurables-axis-space)) then searched `Configurable`'s 24 jail/build/auction/trade combinations against the 5 named strategies, across 4 ruleset environments (`examples/sweep_*.toml` — no optional rules, trading only, house rules only, everything on), and refined the winner's reserve and valuation — repeated at 5 different base seeds specifically to check which axis choices were robust rather than an artifact of one seed's games. `build.stop_before_hotel: false`, `trade: MonopolyCompleting`, and `reserve: 50` held up cleanly on every seed; `jail` and `auction` flipped between seeds with margins small enough to call inconclusive, so `buy_optimal` (`crates/engine/src/strategies/buy_optimal.rs`) hardcodes whichever tied for the win most often (Buy Shrewd's own jail/auction policies, as it happens) and says so honestly rather than presenting a tiebreak as a proven result — see [strategy-search-results.md](./strategy-search-results.md) for the full per-ruleset rankings, the multi-seed comparison, and further caveats (the two no-trading rulesets' raw win rates look tiny purely because most games in them never finish at all, not because the winning strategy plays badly).

**Demo**: `cargo run -p monopoly-cli --release -- sweep --seed 1 --out-dir sweep_results` reproduces one seed of the search from scratch in about 10 seconds (measured, 8 cores) and writes the ranked CSVs and summary [strategy-search-results.md](./strategy-search-results.md) is built from.
