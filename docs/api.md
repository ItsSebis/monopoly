# Server API

The `server` crate (axum + SQLite) is the durable archive and the dispatcher for browser-requested batch runs. It is optional infrastructure: the CLI and a live browser game both work with no server running at all (see [headless-cli.md](./headless-cli.md) and [architecture.md](./architecture.md#native-vs-wasm-two-execution-paths-one-engine)). Its job starts once you want a run remembered past the current session, or want the browser to trigger a native-speed batch it can't run itself.

All request/response bodies use the schemas from [data-model.md](./data-model.md).

## `POST /runs`

Submit a completed run for archiving. Used by:
- The CLI, when invoked with `--archive-url` (see [headless-cli.md](./headless-cli.md)).
- The browser, after a live single game finishes, if the user chooses to save it.

Body: a `Run record` ([data-model.md](./data-model.md#run-record)) — either `kind: "single"` (full events) or `kind: "batch"` (seeds + summaries only). Returns the assigned `id`.

## `POST /runs/batch`

Ask the server to **run** a batch natively (not just archive one already run elsewhere) — this is what the browser calls when the user launches a batch from the UI, since the browser itself never executes batches (see [architecture.md](./architecture.md#native-vs-wasm-two-execution-paths-one-engine)).

Body: `{ "rule_set": RuleSet, "players": [PlayerConfig], "game_count": 10000 }` (seeds are generated server-side from a fresh random base seed, then recorded in the resulting `Run`). This is a longer-running request; the server runs it synchronously for modest counts and returns the completed `Run`, and streams progress (`GET /runs/{id}/status`, simple polling) for large counts so the UI can show a progress bar rather than hanging.

## `GET /runs`

List archived runs, most recent first. Supports basic filtering (`?kind=batch`, `?strategy=buy_good`) for the history browser's search ([frontend.md](./frontend.md#history-browser)). Returns lightweight summaries (id, kind, created_at, rule_set fingerprint, top-line result), not full bodies.

## `GET /runs/{id}`

Fetch one run's full record: for `single`, includes the full event log; for `batch`, includes `per_game_summary` and `aggregate_stats` but not per-game event logs (those are never stored — see below).

## `GET /runs/{id}/games/{seed}`

For a `batch` run only: regenerate and return the full event log for one specific game in the batch, by re-running the engine natively with that batch's `rule_set`/`players` and the given `seed`. This is the "on-demand replay" side of the storage optimization in [architecture.md](./architecture.md#determinism-as-a-storage-optimization) — it lets the UI drill from an aggregate batch view into a single game's full playback without the server ever having stored that game's events.

## `DELETE /runs/{id}`

Remove a run from the archive (used by the history browser's cleanup action).

## Error shape

All errors return `{ "error": "message" }` with a standard HTTP status; there's no bespoke error envelope beyond that.
