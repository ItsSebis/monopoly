# Server API

The `server` crate (axum + SQLite) is the durable archive and the dispatcher for browser-requested batch runs. It is optional infrastructure: the CLI and a live browser game both work with no server running at all (see [headless-cli.md](./headless-cli.md) and [architecture.md](./architecture.md#native-vs-wasm-two-execution-paths-one-engine)). Its job starts once you want a run remembered past the current session, or want the browser to trigger a native-speed batch it can't run itself.

All request/response bodies use the schemas from [data-model.md](./data-model.md).

## `POST /runs`

Submit a completed run for archiving. Used by:
- The CLI, when invoked with `--archive-url` (see [headless-cli.md](./headless-cli.md)).
- The browser, after a live single game finishes, if the user chooses to save it.

Body: `engine::run_record`'s `SingleRunRecord` or `BatchRunRecord` shape ([data-model.md](./data-model.md#run-record)) — i.e. a `Run record` *without* `id`/`kind`/`created_at`, since the server assigns those on ingest. The two shapes are disambiguated by which required fields are present (`seed`/`events` for a single run, `seeds`/`per_game_summary` for a batch) rather than an explicit `kind` field in the request. Returns the full archived `Run record` (the submitted body plus the assigned `id`/`kind`/`created_at`) — the CLI's `--archive-url` only reads `id` off of it, but callers that want the rest (e.g. the browser) don't need a follow-up `GET`.

## `POST /runs/batch`

Ask the server to **run** a batch natively (not just archive one already run elsewhere) — this is what the browser calls when the user launches a batch from the UI, since the browser itself never executes batches (see [architecture.md](./architecture.md#native-vs-wasm-two-execution-paths-one-engine)).

Body: `{ "rule_set": RuleSet, "players": [PlayerConfig], "game_count": 10000 }` (seeds are generated server-side from a fresh random base seed, then recorded in the resulting `Run`). The server runs it synchronously and returns the completed, archived `Run` — every count, not just "modest" ones, as of Phase 4. **`GET /runs/{id}/status` progress polling is deferred to Phase 6**: it has no real consumer until the browser's batch flow needs a progress bar, and would mean threading a progress callback through `run_batch`'s `rayon` loop for a feature nothing exercises yet.

## `GET /runs`

List archived runs, most recent first. Supports basic filtering (`?kind=batch`, `?strategy=buy_good`) for the history browser's search ([frontend.md](./frontend.md#history-browser)). Returns lightweight summaries — `{id, kind, created_at, rule_set, players, winner}` (single) or `{id, kind, created_at, rule_set, players, games}` (batch) — not full bodies. A richer "top-line result" preview (e.g. leading win-rate strategy) is deferred to whatever the Phase 6 history browser actually needs to render.

## `GET /runs/{id}`

Fetch one run's full record: for `single`, includes the full event log; for `batch`, includes `per_game_summary` and `aggregate_stats` but not per-game event logs (those are never stored — see below).

## `GET /runs/{id}/games/{seed}`

For a `batch` run only: regenerate and return a full `SingleRunRecord` (events + `final_stats`) for one specific game in the batch, by re-running the engine natively with that batch's `rule_set`/`players` and the given `seed`. This is the "on-demand replay" side of the storage optimization in [architecture.md](./architecture.md#determinism-as-a-storage-optimization) — it lets the UI drill from an aggregate batch view into a single game's full playback without the server ever having stored that game's events. 400 if `id` isn't a batch run, or `seed` isn't one of that batch's seeds.

## `DELETE /runs/{id}`

Remove a run from the archive (used by the history browser's cleanup action). Returns 204; 404 if `id` doesn't exist.

## Error shape

All errors return `{ "error": "message" }` with a standard HTTP status; there's no bespoke error envelope beyond that.

## Implementation notes (Phase 4)

- Storage is a single SQLite table (`id`, `kind`, `created_at`, a denormalized `strategies` column for the `?strategy=` filter, and the full archived JSON) with one `CREATE TABLE IF NOT EXISTS` at startup — no migrations tool, no ORM. A single connection behind a mutex, not a pool: this is a local, single-user archive with no concurrent-writer load to plan around.
- No authentication and a permissive CORS policy — nothing in this doc set describes a hosted, multi-user deployment; the whole system is local-first. Both are non-goals to revisit only if that ever changes.
