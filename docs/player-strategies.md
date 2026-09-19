# Player Strategies

A strategy implements the `Strategy` trait from [simulation-engine.md](./simulation-engine.md#the-strategy-trait). Every decision point a human player faces is covered, not just purchasing — this matters because purchase-only strategies would converge to nearly identical outcomes once monopolies form; the interesting differences between strategies show up in *building*, *jail*, and *mortgage* behavior just as much as in buying.

## Built-in strategies

### Buy All

Buys every property it can afford at the moment of landing, keeping only a small safety cash reserve (configurable, default $50) to cover near-term rent risk. Fully committed to accumulating property; will mortgage a property to build on the same or another group rather than sit on cash. In jail, always tries to leave as fast as possible (pay the fine immediately if it has one, else use a card, else pay rather than risk missed turns) so it can keep landing on and building up properties.

### Buy Good

Buys based on a value heuristic rather than "can afford it": each candidate property is scored on (a) rent-to-price ratio, (b) how close it puts the player to completing a monopoly, and (c) statistical landing frequency (properties past Jail — the orange and red groups — are landed on disproportionately often due to the "just visiting" bounce, and are weighted up). Only buys when the score clears a threshold *and* a cash reserve (default $150) is maintained afterward. Builds houses only once a full monopoly is held and cash reserve allows it, following the even-build rule. In jail, stays if it holds few or no monopolies (waiting out variance costs little), but pays to leave promptly once it holds a monopoly worth actively collecting rent on.

### Buy Bad

A deliberately suboptimal baseline, included so batch analysis has a clear "worse" reference point to measure the other strategies against (see [analysis-and-metrics.md](./analysis-and-metrics.md#strategy-head-to-head-matrix)). Prioritizes expensive, low rent-to-price properties (the inverse of Buy Good's heuristic) and ignores monopoly completion. Overspends relative to its cash reserve, making it more building- and mortgage-averse later in the game simply because it has less to work with. In jail, rolls for doubles by default (free, but slower) rather than paying, even when it can afford to leave sooner.

### Buy None

Never buys, never bids in auctions, never builds. Used as a floor-line control (a game full of Buy None strategies effectively never ends by bankruptcy through rent, only through configured tax/luxury attrition, which is itself a useful sanity signal for validating the engine). Still makes jail decisions using the same logic as Buy Good, since jail behavior isn't tied to ownership.

## Decision summary by strategy

| Decision | Buy All | Buy Good | Buy Bad | Buy None |
|---|---|---|---|---|
| Purchase | Always, if reserve allows | Threshold heuristic | Inverse heuristic | Never |
| Auction bid | Up to affordability minus reserve | Up to heuristic value | Overbids on poor properties | Always abstains |
| Build | ASAP once monopoly held | Once monopoly held, reserve-gated | Rarely (cash-poor) | Never |
| Mortgage | Freely, to fund building | Only to avoid bankruptcy | Only to avoid bankruptcy | N/A (owns nothing) |
| Jail | Leave ASAP | Stay early, leave once profitable | Roll for doubles (default) | Same as Buy Good |

## Custom strategies

Anything implementing the `Strategy` trait can be used interchangeably with the built-ins — by the CLI (`--strategy` referencing a registered implementation or a scripted config), by the server for batch dispatch, and by the browser's strategy picker (which lists whatever strategies the running engine build has registered). A custom strategy only needs to implement the hooks it cares about differently; sensible defaults (mirroring Buy Good) are provided for the rest via default trait methods, so e.g. a strategy that only changes jail behavior doesn't need to reimplement purchasing.

Parameterized variants (e.g. "Buy Good with a $300 reserve" instead of $150) are expressed as config on top of a base strategy rather than as wholly new types — see `StrategyConfig` in [data-model.md](./data-model.md#playerconfig).
