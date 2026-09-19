# Frontend

The `web/` project (TypeScript + Vite) is an interface layer only — it contains no Monopoly rules. Every decision it needs (what happens when a player lands somewhere, whether a purchase is accepted) is made by the engine, either locally via WASM (live games) or remotely via the server API (batch/history). This keeps the "browser is just an interface" constraint concrete rather than aspirational: if you deleted `web/` entirely, every capability of the simulator would still exist via the CLI.

## Structure

```
web/src/
  board/       # renders the 40-space board + player tokens from a GameState snapshot
  controls/    # config forms (RuleSet + PlayerConfig editor) and playback controls
  worker/      # a Web Worker wrapping engine-wasm — simulation runs off the main thread
  charts/      # analysis dashboard (Chart.js), consumes final_stats/aggregate_stats
  history/     # history browser: calls the server API, falls back to a localStorage cache
```

## Config forms → engine input

The config screen is a direct editor for `RuleSet` and a list of `PlayerConfig` ([data-model.md](./data-model.md)) — every field in those schemas has a corresponding form control (toggle, number input, or strategy dropdown). There's no intermediate UI-specific config format; submitting the form produces exactly the JSON the engine (via WASM or the API) expects.

## Live playback

1. On "start", the main thread posts `{ rule_set, players, seed }` to the Worker, which constructs a `Game` via `engine-wasm` and calls `step_turn()` in a loop, posting each turn's events back to the main thread.
2. The main thread buffers incoming events and drains them on a timer whose interval is derived from the speed control (a multiplier, e.g. 1x/4x/32x/"instant"), applying each event to the board renderer as it's drained. This is why speed control needs no engine support at all (see [simulation-engine.md](./simulation-engine.md#turn-state-machine)) — it's purely how fast the UI consumes an already-produced event stream.
3. Pause stops draining (the Worker may keep simulating ahead, or the main thread can flow-control it — an implementation detail, not a contract); step-forward drains exactly one event.
4. On game end, the UI offers to save the run (`POST /runs` to the server) and always shows the local analysis dashboard immediately, using the events already in memory — saving is for persistence, not required to see results.

## Batch runs from the UI

The config screen has a "run N games" mode instead of "watch live". Submitting it calls `POST /runs/batch` on the server ([api.md](./api.md#post-runsbatch)); the UI shows a progress indicator and then renders the aggregate dashboard from the returned `aggregate_stats`. No WASM execution happens for this path (see [architecture.md](./architecture.md#native-vs-wasm-two-execution-paths-one-engine)) — the UI is purely submitting a request and rendering a result.

## History browser

- `localStorage` keeps a small list of recently-viewed run summaries (id, created_at, top-line result) for instant display without a network round-trip — this is a cache, not a source of truth.
- The actual list/detail views call the server API (`GET /runs`, `GET /runs/{id}`) directly; if no server is reachable, the history browser degrades to showing only the `localStorage` cache with a note that the archive is unavailable, rather than failing outright.
- Drilling into one game from a batch result calls `GET /runs/{id}/games/{seed}` and then reuses the same board renderer as live playback to show that specific game's full turn-by-turn replay.

## Charts

The dashboard renders every metric in [analysis-and-metrics.md](./analysis-and-metrics.md) from whatever `final_stats`/`aggregate_stats` came back with the run — single-run and batch-run dashboards share components wherever a metric applies to both (e.g. net-worth-over-time renders one line set for a single run, or a percentile band for a batch).
