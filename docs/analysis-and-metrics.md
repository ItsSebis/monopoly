# Analysis & Metrics

Every metric here is computed from the event log ([simulation-engine.md](./simulation-engine.md#event-log)) by `engine::stats::compute_stats` — nothing is tracked by a separate, parallel bookkeeping system, so single-run (`PerGameStats`) and batch (`AggregateStats`, in `engine::batch`) analysis share the same per-game replay, just consumed differently: a single run keeps the full per-turn detail, a batch folds each game's contribution in and discards the rest (see [simulation-engine.md](./simulation-engine.md#batch-execution)).

## Per-run metrics (`PerGameStats`, from `monopoly run`'s `final_stats`)

| Metric | Description | Chart |
|---|---|---|
| Winner / final standings | Who won, and each bankruptcy's turn and payee | Table (single) / win-rate bar chart by player-strategy (batch) |
| Net worth over time | Cash + property price + building value, one row per turn boundary, per player | Line chart (single: one line per player) |
| Cash flow breakdown | Total paid/received by category: rent, tax, card effects, GO salary | Stacked bar per player |
| Property & monopoly timeline | When each property was bought (direct purchase or auction win), and when each color group's monopoly completed (including via a bankruptcy transfer) | Timeline / Gantt-style chart |
| Bankruptcy cause & turn | Who went bankrupt, on what turn, and to whom (player or bank) | Table (single) / histogram of bankruptcy turn (batch) |
| Dice roll / landing distribution | Actual frequency of each roll total (2-12) and each landed-on space | Histogram / board heatmap overlay |
| Game length | Turns until one player remained (or `max_turns`/the internal safety cap was hit) | Single number (single run) / distribution histogram (batch) |
| ROI by property | Rent collected vs. cost basis (list price plus any houses/hotel built, at game end), per property | Bar chart, sorted descending |

**Deliberate Phase 3 scoping** (see `crates/engine/src/stats.rs`'s module doc comment for the mechanics):
- **Dice-roll "expected" baseline** isn't computed — only actual counts. A true expected-landing-frequency baseline needs the board's full Markov chain (dice + card movement + jail transitions), which has no consumer until the Phase 6 heatmap renders it; the 2d6 total's closed-form expected distribution is a fixed, well-known table needing no engine support.
- **ROI by property** attributes a property's whole-game rent to its *final* owner and *final* house count, even if it changed hands (bankruptcy-to-player) or was partially sold down (a raise-cash sale) mid-game — a documented approximation, not a per-owner or per-building-state split.
- **Landing distribution** counts a `Move` event's destination — sent-to-Jail (`GoToJailSpace`/`ThreeDoubles`/a card) reassigns position without a `Move` event, so Jail's count reflects "just visiting" landings only, not total arrivals. Compare against the classic Monopoly frequency chart with that caveat in mind.
- **The Free Parking pot's payout is a known gap**: house rule enabled, the pot's payout (`Game::pay_from_bank` from `resolve_landing`'s `FreeParking` arm) isn't independently event-logged, so it can't be replayed — every number here is exact only under the official rule (`free_parking_pot: false`, the default).

## Batch-only metrics (`AggregateStats`, from `monopoly batch`)

| Metric | Description | Chart |
|---|---|---|
| Win rate by strategy | Win % for each strategy present, counted per **seat** (a strategy holding 2 of 4 seats is counted twice per game in the denominator, keeping its rate comparable to a single-seat strategy) | Bar chart |
| Strategy head-to-head matrix | Pairwise win rate when strategy A faced strategy B in the same game, counted only for games where the winner was one of the two (a third strategy winning doesn't resolve that pairing) | Heatmap matrix — this is what quantifies the [Buy Bad](./player-strategies.md#buy-bad) baseline: every other strategy's cell against it should show a clearly higher win rate, which doubles as a sanity check that the engine and strategies behave as designed. When two seats share a strategy, that strategy's diagonal cell is 50% by construction (each mirror-matchup game records both a win and a loss into it) — not comparable to an off-diagonal cell. |
| ROI by strategy | Total rent collected divided by total cost basis, summed across every property each strategy owned at game end (not an average of per-property ratios) | Bar chart |
| Final net worth by strategy | Mean/median/p10/p90 of final net worth, grouped by strategy | Box-and-whisker or grouped bar |
| Game length / bankruptcy-turn / dice-roll / landing distributions | The per-run versions above, pooled across every game in the batch | Histograms / heatmap |
| Rule-variant comparison | Same strategy mix run as two separate `monopoly batch` invocations under different `RuleSet`s (e.g. `free_parking_pot` on vs. off), diffed by the caller — no dedicated feature or field, since it needs no new data, just two runs | Grouped bar chart |

**Deliberate Phase 3 scoping**: unlike the per-run net-worth-over-time line chart, the batch aggregate only keeps each game's *final* net worth, not a full per-turn series. Aggregating variable-length trajectories from thousands of games into a true time-aligned percentile band needs a resampling convention (by turn number? by % of game elapsed?) that's more sensibly decided in Phase 6, once a chart actually needs to consume it.

**Batches with no `max_turns` set**: strategy mixes that don't reliably terminate without trading (see [game-rules.md](./game-rules.md#bankruptcy)) can leave a large fraction of a batch undecided (`winner: null`, `turns` at the internal safety cap) — those games still contribute to game-length/dice/landing distributions but not to win rate or head-to-head. Setting `RuleSet.max_turns` avoids this and bounds batch cost (see [headless-cli.md](./headless-cli.md)).

## Board heatmap

A recurring visualization across both categories: the 40-space board rendered with each space shaded by a chosen metric — landing frequency, total rent generated, or ROI. Reuses the same board layout component as live playback ([frontend.md](./frontend.md#live-playback)), just with a color overlay instead of player tokens. Not yet rendered anywhere (Phase 6); the underlying counts (`landing_counts`, `property_roi`) already exist as of Phase 3.

## Where this lives

- Computed once per game inside `engine::stats::compute_stats` (not duplicated in the UI) and included as `final_stats`/`aggregate` in the [Run record](./data-model.md#run-record), so the CLI's JSON/CSV output and the browser's charts are reading the exact same numbers.
- Rendered by `web/src/charts` ([frontend.md](./frontend.md#charts)) using Chart.js; the CLI's `--csv` path is the escape hatch for anyone who wants to chart these numbers in an external tool instead.
