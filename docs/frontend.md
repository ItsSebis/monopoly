# Frontend

The `web/` project (TypeScript + Vite) is an interface layer only — it contains no Monopoly rules. Every decision it needs (what happens when a player lands somewhere, whether a purchase is accepted) is made by the engine, either locally via WASM (live games) or remotely via the server API (batch/history). This keeps the "browser is just an interface" constraint concrete rather than aspirational: if you deleted `web/` entirely, every capability of the simulator would still exist via the CLI.

## Structure

```
web/src/
  board/       # renders the 40-space board + player tokens from a GameState snapshot
  controls/    # config forms (RuleSet + PlayerConfig editor) and playback controls
  worker/      # a Web Worker wrapping engine-wasm — simulation runs off the main thread
  eventLog.ts  # formats an event into a human-readable line (Phase 5)
  types.ts     # TS mirror of the JSON shapes engine serializes (Phase 5)
  main.ts      # wires the config form, worker, board, and playback controls together (Phase 5)
  charts/      # analysis dashboard (Chart.js), consumes final_stats/aggregate_stats (Phase 6)
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

## Live playback

1. On "start", the main thread posts `{ config, seed }` (`config` a full `GameConfig` — `rule_set` + `players`) to the Worker, which constructs a `WasmGame` and calls `step_turn()` in a loop until `is_over()`, posting each turn's `{ events, state }` back to the main thread as it goes — the Worker always races ahead of whatever the main thread has actually drained (implemented as the simplest correct choice; the alternative, the main thread flow-controlling the Worker, was left unimplemented since nothing in Phase 5 needs it).
2. The main thread buffers incoming events into one flat queue and drains them individually on a timer whose interval is derived from the speed control (a multiplier, e.g. 1x/4x/32x/"instant"), applying each event to the board renderer as it's drained. This is why speed control needs no engine support at all (see [simulation-engine.md](./simulation-engine.md#turn-state-machine)) — it's purely how fast the UI consumes an already-produced event stream. Per-event animation is limited to moving a player's token (the only per-event visual worth animating); ownership, houses, mortgages, and cash always come from a full re-sync to that turn's authoritative `GameState`, applied once every one of that turn's events has drained — the engine's own state is always the ground truth, never something reconstructed by interpreting events.
3. Pause stops draining (the Worker keeps simulating ahead); step-forward drains exactly one event regardless of play/pause state.
4. **`Event::GameEnded` is only ever recorded by `Game::run_to_completion`, never by `step_turn`** — live playback drives `step_turn` directly, so it never sees that event. Once `is_over()` is true, the Worker instead derives the winner the same way `run_to_completion` does (the one non-bankrupt player, if exactly one remains) from the final `state`, and the main thread shows the winner banner once playback has actually drained through to that point (not the instant the Worker itself finishes, which can be well ahead at slower speeds).
5. **Phase 5 does not offer to save the run.** That needs the server, and Phase 5's own demo bar is "zero server running" — wiring `POST /runs` into this flow is Phase 6's job, alongside the rest of the browser's server integration (batch + history). The local analysis dashboard mentioned above is also Phase 6 (see [Charts](#charts)); Phase 5's UI is the board, the event log, and playback controls only.

Purely cosmetic board data — space names, colors, and grid position — has no engine-side representation (confirmed: `Board`/`SpaceKind` carry only price/rent/group, never a display name); `board/layout.ts` hardcodes a 40-entry table keyed by the same space index the engine uses everywhere. This is a deliberate line: the engine stays the single source of truth for anything a *decision* depends on, but nothing depends on a space's display name.

## Batch runs from the UI

The config screen has a "run N games" mode instead of "watch live". Submitting it calls `POST /runs/batch` on the server ([api.md](./api.md#post-runsbatch)); the UI shows a progress indicator and then renders the aggregate dashboard from the returned `aggregate_stats`. No WASM execution happens for this path (see [architecture.md](./architecture.md#native-vs-wasm-two-execution-paths-one-engine)) — the UI is purely submitting a request and rendering a result.

## History browser

- `localStorage` keeps a small list of recently-viewed run summaries (id, created_at, top-line result) for instant display without a network round-trip — this is a cache, not a source of truth.
- The actual list/detail views call the server API (`GET /runs`, `GET /runs/{id}`) directly; if no server is reachable, the history browser degrades to showing only the `localStorage` cache with a note that the archive is unavailable, rather than failing outright.
- Drilling into one game from a batch result calls `GET /runs/{id}/games/{seed}` and then reuses the same board renderer as live playback to show that specific game's full turn-by-turn replay.

## Charts

The dashboard renders every metric in [analysis-and-metrics.md](./analysis-and-metrics.md) from whatever `final_stats`/`aggregate_stats` came back with the run — single-run and batch-run dashboards share components wherever a metric applies to both (e.g. net-worth-over-time renders one line set for a single run, or a percentile band for a batch).
