# Analysis & Metrics

Every metric here is computed from the event log ([simulation-engine.md](./simulation-engine.md#event-log)) — nothing is tracked by a separate, parallel bookkeeping system, so single-run and batch analysis share the same aggregation code, just run over one game or averaged/percentiled over many.

## Per-run metrics (every run, single or batch-aggregated)

| Metric | Description | Chart |
|---|---|---|
| Winner / final standings | Who won, and rank order of when each other player went bankrupt | Table (single) / win-rate bar chart by player-strategy (batch) |
| Net worth over time | Cash + property price + building value, sampled per turn, per player | Line chart (single: one line per player; batch: median + percentile band per strategy) |
| Cash flow breakdown | Total paid/received by category: rent, tax, card effects, GO salary | Stacked bar per player |
| Property & monopoly timeline | When each property was bought, and when each monopoly completed | Timeline / Gantt-style chart |
| Bankruptcy cause & turn | Who went bankrupt, on what turn, and to whom (player or bank) | Table (single) / histogram of bankruptcy turn (batch) |
| Dice roll distribution ("luck") | Actual vs. statistically expected frequency of each roll total and each landed-on space | Histogram / board heatmap overlay |
| Game length | Turns until one player remained (or the `max_turns` cap was hit) | Single number (single run) / distribution histogram (batch) |
| ROI by property | Total rent collected vs. purchase + building cost, per property | Bar chart, sorted descending |

## Batch-only metrics

| Metric | Description | Chart |
|---|---|---|
| Win rate by strategy | Across all games in the batch, win % for each strategy present | Bar chart |
| Strategy head-to-head matrix | Pairwise win rate when strategy A faced strategy B in the same game | Heatmap matrix — this is what quantifies the [Buy Bad](./player-strategies.md#buy-bad) baseline: every other strategy's cell against it should show a clearly higher win rate, which doubles as a sanity check that the engine and strategies behave as designed |
| ROI by strategy | Average ROI-by-property (above) grouped by which strategy owned the property | Bar chart |
| Rule-variant comparison | Same strategy mix run under different `RuleSet`s (e.g. free-parking-pot on vs. off) to show how a house rule shifts outcomes | Grouped bar chart |

## Board heatmap

A recurring visualization across both categories: the 40-space board rendered with each space shaded by a chosen metric — landing frequency, total rent generated, or ROI. Reuses the same board layout component as live playback ([frontend.md](./frontend.md#live-playback)), just with a color overlay instead of player tokens.

## Where this lives

- Computed once per game inside `engine`'s batch/single-run aggregation step (not duplicated in the UI) and included as `final_stats`/`aggregate_stats` in the [Run record](./data-model.md#run-record), so the CLI's JSON/CSV output and the browser's charts are reading the exact same numbers.
- Rendered by `web/src/charts` ([frontend.md](./frontend.md#charts)) using Chart.js; the CLI's `--out results.csv` path is the escape hatch for anyone who wants to chart these numbers in an external tool instead.
