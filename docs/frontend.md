# Frontend

The `web/` project (TypeScript + Vite) is an interface layer only — it contains no Monopoly rules. Every decision it needs (what happens when a player lands somewhere, whether a purchase is accepted) is made by the engine, either locally via WASM (live games) or remotely via the server API (batch/history). This keeps the "browser is just an interface" constraint concrete rather than aspirational: if you deleted `web/` entirely, every capability of the simulator would still exist via the CLI.

## Structure

```
web/src/
  board/       # renders the 40-space board + player tokens from a GameState snapshot
  controls/    # the config form (RuleSet + PlayerConfig editor, live/batch mode toggle),
               # playback controls, and batch-results rendering (Phase 6)
  worker/      # a Web Worker wrapping engine-wasm — simulation runs off the main thread
  eventLog.ts  # formats an event into a human-readable line (Phase 5)
  types.ts     # TS mirror of the JSON shapes engine serializes, incl. stats/Run-record shapes (Phase 6)
  main.ts      # wires the config form, worker, board, playback, batch, and history together
  api.ts       # thin fetch wrapper over the server's six endpoints; the persisted server-URL setting (Phase 6)
  charts/      # analysis dashboard (Chart.js + dependency-light substitutes), consumes final_stats/aggregate_stats (Phase 6)
  history/     # history browser: calls the server API, falls back to a localStorage cache (Phase 6)
```

Plain TypeScript + Vite, no UI framework — the project's small-surface-area bias and a DOM-rendered (not canvas) board keep the whole thing easy to inspect and style directly.

### Development setup

Building `engine-wasm` needs a couple of one-time steps beyond `npm install`:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

`npm run dev`/`npm run build` in `web/` regenerate the wasm bindings automatically first (`predev`/`prebuild` run `wasm-pack build ../crates/engine-wasm --target web --out-dir ../../web/src/wasm` — note `--out-dir` is resolved relative to the *crate* path, not the invoking shell's directory, which is easy to get wrong).

## Config forms → engine input

The config screen is a direct editor for `RuleSet` and a list of `PlayerConfig` ([data-model.md](./data-model.md)) — every field in those schemas has a corresponding form control (toggle, number input, or strategy dropdown). There's no intermediate UI-specific config format; submitting the form produces exactly the JSON the engine (via WASM or the API) expects.

**One form, two modes.** Rather than duplicate the Rules/Players fieldsets across a separate "live" and "batch" screen, the config form has a `run_mode` toggle: "live" shows a seed field and submits to `startReplay` (below); "batch" shows a game-count field and submits to [`POST /runs/batch`](#batch-runs-from-the-ui) instead. Both modes share the exact same `buildRuleSet`/`buildPlayers` mapping — a batch run and a live game are the same `RuleSet + PlayerConfig[]`, just executed differently.

## Live playback

1. On "start", the main thread posts `{ config, seed }` (`config` a full `GameConfig` — `rule_set` + `players`) to the Worker, which constructs a `WasmGame` and calls `step_turn()` in a loop until `is_over()`, posting each turn's `{ events, state }` back to the main thread as it goes — the Worker always races ahead of whatever the main thread has actually drained (implemented as the simplest correct choice; the alternative, the main thread flow-controlling the Worker, was left unimplemented since nothing in Phase 5 needs it).
2. The main thread buffers incoming events into one flat queue and drains them individually on a timer whose interval is derived from the speed control (a multiplier, e.g. 1x/4x/32x/"instant"), applying each event to the board renderer as it's drained. This is why speed control needs no engine support at all (see [simulation-engine.md](./simulation-engine.md#turn-state-machine)) — it's purely how fast the UI consumes an already-produced event stream. Per-event animation is limited to moving a player's token (the only per-event visual worth animating) — `BoardView.moveToken` does a FLIP transform (capture the old bounding rect, reparent, animate from an inverse transform back to identity) so the token visibly glides rather than snapping, degrading gracefully to an instant move at very high speeds; ownership, houses, mortgages, and cash always come from a full re-sync to that turn's authoritative `GameState`, applied once every one of that turn's events has drained — the engine's own state is always the ground truth, never something reconstructed by interpreting events.
3. Pause stops draining (the Worker keeps simulating ahead); step-forward drains exactly one event regardless of play/pause state.
4. **`Event::GameEnded` is only ever recorded by `Game::run_to_completion`, never by `step_turn`** — live playback drives `step_turn` directly, so it never sees that event. Once `is_over()` is true, the Worker instead derives the winner the same way `run_to_completion` does (the one non-bankrupt player, if exactly one remains) from the final `state`, and the main thread shows the winner banner once playback has actually drained through to that point (not the instant the Worker itself finishes, which can be well ahead at slower speeds).
5. **"View stats" and "Save run" (Phase 6)** appear next to the winner banner once the game ends. Neither replays or tracks the live game's own event stream — both ask the Worker to call the `engine-wasm` export `build_single_run_record(config, seed)`, which deterministically re-simulates the exact same game natively (well under a millisecond) and returns the canonical `SingleRunRecord` — the same shape `GET /runs/{id}/games/{seed}` returns. "View stats" renders it locally with [Charts](#charts) (no server needed, preserving the "zero server running" bar for plain live play); "Save run" additionally `POST`s it to `/runs` ([api.md](./api.md#post-runs)) and adds it to the [history browser's](#history-browser) local cache.

Purely cosmetic board data — space names, colors, and grid position — has no engine-side representation (confirmed: `Board`/`SpaceKind` carry only price/rent/group, never a display name); `board/layout.ts` hardcodes a 40-entry table keyed by the same space index the engine uses everywhere. This is a deliberate line: the engine stays the single source of truth for anything a *decision* depends on, but nothing depends on a space's display name.

**Board-only language toggle (Phase 7)**: `layout.ts` carries a German name (`nameDe`) alongside each space's English one, plus `BoardLang`/`getBoardLang`/`setBoardLang`/`displayName` (`localStorage`-backed, mirroring `api.ts`'s server-URL setting). A `<select>` next to the Speed control calls `BoardView.setLanguage()`, which updates only its own space-name elements. Scoped to the board deliberately: the event log (`eventLog.ts`) and every chart label (`spaceName()`) keep using English regardless of this setting, since the toggle is about the board display, not the stats dashboard.

## One replay pipeline for every "watch a game" need

Since a game is fully determined by `(rule_set, players, seed)`, `startReplay()` in `main.ts` is the single entry point that drives a Worker/`WasmGame`/`PlaybackController`/`BoardView` playback — used identically by a fresh "Start", a batch drilldown ([below](#batch-runs-from-the-ui)), and a history "Replay on board" ([below](#history-browser)). There is no second renderer that reconstructs `GameState` from a raw event log: a batch's `GET /runs/{id}/games/{seed}` and a history run's stored `events` are available, but the board always replays by re-simulating locally instead, since it's already deterministic, instant, and reuses Phase 5's tested pipeline verbatim rather than teaching `board.ts` to interpret an archived event log a second time.

## Batch runs from the UI

Selecting "batch" mode on the config form ([above](#config-forms--engine-input)) submits to `POST /runs/batch` on the server ([api.md](./api.md#post-runsbatch)) instead of starting a live game; no WASM execution happens for this path (see [architecture.md](./architecture.md#native-vs-wasm-two-execution-paths-one-engine)).

**The "progress indicator" is just a disabled submit button.** `docs/api.md`'s `POST /runs/batch` section records why: a 5,000-game batch finishes in 0.17s server-side, well before a progress poll could land, so `GET /runs/{id}/status` was deliberately never built — there's no real progress to report. The whole request is one synchronous `fetch`.

The response's `aggregate_stats` renders through [Charts](#charts); its `per_game_summary` becomes a table with a "Replay" button per game, which calls `startReplay()` with the batch's own `(rule_set, players, seed)` — see [above](#one-replay-pipeline-for-every-watch-a-game-need).

## History browser

- `localStorage` (`history/recentRunsCache.ts`) keeps a small, capped (20-entry) list of recently-viewed run summaries for instant display without a network round-trip — this is a cache, not a source of truth. It's written whenever a run is saved, opened, or created (a batch run).
- The list view calls `GET /runs` directly; if the server can't be reached, it falls back to rendering only the `localStorage` cache with a "server unreachable" note, rather than failing outright.
- The detail view calls `GET /runs/{id}` and renders the same [Charts](#charts) components as "View stats", plus a "Replay on board" button (single runs) or the same per-game replay table as a fresh batch result (batch runs) — both going through `startReplay()`.
- A delete button calls `DELETE /runs/{id}` and removes the entry from the local cache too.

## Charts

`charts/` renders every metric in [analysis-and-metrics.md](./analysis-and-metrics.md) from whatever `final_stats`/`aggregate_stats` came back with the run — `singleRunCharts.ts` and `batchCharts.ts` share the small `barChart.ts`/`lineChart.ts`/`histogram.ts`/`heatmap.ts`/`domHelpers.ts`/`commonSections.ts` building blocks (the dice-roll and landing-distribution sections are identical between the two, so `commonSections.ts` renders both). Chart.js (bar, stacked bar, line) covers every metric it has a native type for; three metrics get a dependency-light substitute instead of a second charting library:

- **Property/monopoly timeline** ("Timeline/Gantt-style" per the docs) renders as a plain sorted table — adding a Gantt library for one table-shaped view isn't worth it.
- **Strategy head-to-head matrix** and the **board heatmap** (dice/landing distribution overlay) render as CSS-colored grids (`charts/heatmap.ts`'s `colorFor` scale) rather than a Chart.js matrix plugin; the heatmap reuses `board/layout.ts`'s 40-space table with no tokens/ownership, since it's a read-only overlay, not a game in progress. `colorFor` stretches its lightness range across the series' actual `[min, max]` (Phase 7), not a fixed 0-based floor, so a narrow-range series (landing counts rarely near 0) still spans the full light-to-dark scale instead of clustering in one shade; the head-to-head matrix's own 0-100% win-rate scale is unaffected (it never passes a non-zero `min`).
- **Final net worth by strategy** ("box-and-whisker" per the docs) renders as a grouped bar of `{p10, median, mean, p90}` — exactly the fields `AggregateStats::final_net_worth_by_strategy`'s `DistributionSummary` already has, with no box-plot plugin needed.

Rule-variant comparison needs no UI at all: it's two separate batch runs, diffed by the caller, with the CSV/JSON export as the escape hatch — unchanged from [analysis-and-metrics.md](./analysis-and-metrics.md#batch-only-metrics).
