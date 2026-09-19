# Player Strategies

A strategy implements the `Strategy` trait from [simulation-engine.md](./simulation-engine.md#the-strategy-trait). Every decision point a human player faces is covered, not just purchasing — this matters because purchase-only strategies would converge to nearly identical outcomes once monopolies form; the interesting differences between strategies show up in *building*, *jail*, and *mortgage* behavior just as much as in buying.

## Built-in strategies

### Buy All

Buys every property it can afford at the moment of landing, keeping only a small safety cash reserve (configurable, default $50) to cover near-term rent risk. Builds on every monopoly it holds as soon as its reserve allows, cheapest-eligible-property first. In jail, pays the fine immediately if affordable (a held "Get Out of Jail Free" card is always used automatically by the engine before any strategy is even asked — see [game-rules.md](./game-rules.md#jail) — so this only covers what happens without one).

### Buy Good

Buys based on a value heuristic rather than "can afford it": each candidate property is scored on (a) rent-to-price ratio and (b) how close it puts the player to completing a monopoly. Only buys when the score clears a threshold *and* a cash reserve (default $150) is maintained afterward. (A landing-frequency weighting — e.g. valuing the orange/red groups higher since they're statistically landed on more often just past Jail — was considered but isn't implemented; the two-factor heuristic already gives distinct-enough behavior from Buy All.) Builds on its monopolies the same way Buy All does, just with the larger $150 reserve — naturally slower and more conservative without needing separate building logic. In jail, stays if it holds few or no monopolies (waiting out variance costs little), but pays to leave promptly once it holds a monopoly worth actively collecting rent on.

### Buy Bad

A deliberately suboptimal baseline, included so batch analysis has a clear "worse" reference point to measure the other strategies against (see [analysis-and-metrics.md](./analysis-and-metrics.md#strategy-head-to-head-matrix)). Prioritizes expensive, low rent-to-price properties (the inverse of Buy Good's heuristic) and ignores monopoly completion; the same inverted heuristic makes it *overbid* on those same low-value properties at auction. Never builds — its persistently thin cash position (from buying down to a $20 reserve) means it essentially never has the surplus to, which is a faithful approximation of "rarely" without needing separate never-quite-triggers logic. In jail, rolls for doubles by default (free, but slower) rather than paying, even when it can afford to leave sooner.

### Buy None

Never buys, never bids in auctions, never builds, never mortgages (it owns nothing to mortgage). Used as a floor-line control. Still makes jail decisions using the same logic as Buy Good, since jail behavior isn't tied to ownership.

## Decision summary by strategy

| Decision | Buy All | Buy Good | Buy Bad | Buy None |
|---|---|---|---|---|
| Purchase | Always, if reserve allows | Threshold heuristic | Inverse heuristic | Never |
| Auction bid | Up to affordability minus reserve | Up to heuristic value | Overbids on poor properties | Always abstains |
| Build | ASAP once monopoly held, $50 reserve | Same, $150 reserve | Never (see above) | Never (owns nothing) |
| Mortgage / sell houses | Cheapest-first, to avoid bankruptcy | Cheapest-first, to avoid bankruptcy | Cheapest-first, to avoid bankruptcy | N/A (owns nothing) |
| Jail (no card held) | Pay if affordable | Stay early, pay once profitable | Roll for doubles (default) | Same as Buy Good |

A held "Get Out of Jail Free" card is always used automatically for every strategy, before `decide_jail_action` is even called — see [game-rules.md](./game-rules.md#jail).

## Custom strategies

Anything implementing the `Strategy` trait can be used interchangeably with the built-ins — by the CLI (`--strategy` referencing a registered implementation or a scripted config), by the server for batch dispatch, and by the browser's strategy picker (which lists whatever strategies the running engine build has registered). As of Phase 2, the trait has no default method implementations — a custom strategy must implement all five hooks, even a trivial one for a hook it doesn't care about (e.g. `decide_build` returning an empty `Vec`). Default implementations (e.g. mirroring Buy Good) are a reasonable future addition once a real custom strategy actually needs to override only one or two hooks; nothing in Phase 1-2 does.

Parameterized variants (e.g. "Buy Good with a $300 reserve" instead of $150) are expressed as config on top of a base strategy rather than as wholly new types — see `StrategyConfig` in [data-model.md](./data-model.md#playerconfig).
