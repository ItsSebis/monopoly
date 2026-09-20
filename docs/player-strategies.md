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

### Buy Shrewd

Added in Phase 7, combining every gap the "Where the built-in strategies diverge from this" section below names into one strategy, rather than four separate registrations. Otherwise plays like a more aggressive Buy Good (same rent-to-price-plus-monopoly-bonus scoring, threshold, and cheapest-first mortgage behavior; a $100 reserve, between Buy All's and Buy Good's):

- **Landing-frequency-weighted valuation**: every candidate score is multiplied by a static per-group weight (Orange ×1.3, Red ×1.15, everything else ×1.0), from the Markov-chain research cited below — the exact gap Buy Good's own doc comment above says was considered and skipped.
- **House-supply-denial building**: develops every held monopoly up to 4 houses but never converts to a hotel, deliberately keeping the bank's fixed 32-house stock locked up rather than freeing 4 houses back on hoteling.
- **Opponent-hotel-risk jail policy**: leaves jail quickly while no *opponent* holds a built-up monopoly yet (the board's still open), but stays once one does — the mirror image of Buy Good's own-monopoly-based jail logic, keyed on the risk of landing on someone else's hotel rather than on the acting player's own holdings.
- **Auction denial bidding**: if a single other player already owns every other member of a group up for auction, bids up to full affordability regardless of its own valuation, purely to block that player's monopoly.
- **Trading**: uses the same monopoly-completing heuristic Buy All/Buy Good share (see below).

## Decision summary by strategy

| Decision | Buy All | Buy Good | Buy Bad | Buy None | Buy Shrewd |
|---|---|---|---|---|---|
| Purchase | Always, if reserve allows | Threshold heuristic | Inverse heuristic | Never | Threshold heuristic, landing-frequency-weighted |
| Auction bid | Up to affordability minus reserve | Up to heuristic value | Overbids on poor properties | Always abstains | Denial bid if it'd block an opponent's monopoly, else weighted heuristic value |
| Build | ASAP once monopoly held, $50 reserve | Same, $150 reserve | Never (see above) | Never (owns nothing) | ASAP to 4 houses, $100 reserve, never hotels (supply denial) |
| Mortgage / sell houses | Cheapest-first, to avoid bankruptcy | Cheapest-first, to avoid bankruptcy | Cheapest-first, to avoid bankruptcy | N/A (owns nothing) | Cheapest-first, to avoid bankruptcy |
| Jail (no card held) | Pay if affordable | Stay early, pay once profitable | Roll for doubles (default) | Same as Buy Good | Leave while no opponent has built up, stay once one does |
| Trade (`RuleSet.trading_enabled`, Phase 7) | Proposes/accepts monopoly-completing swaps (see below) | Same logic as Buy All | Never proposes or accepts | Never proposes or accepts | Same logic as Buy All |

A held "Get Out of Jail Free" card is always used automatically for every strategy, before `decide_jail_action` is even called — see [game-rules.md](./game-rules.md#jail).

## Real-world strategy research

The built-in strategies are deliberately simple reference points (see above), not an attempt at optimal play. This section summarizes what's actually established online about strong human/simulated Monopoly play — mostly to ground future strategy work (custom strategies, and Phase 7's trading) in real analysis rather than folk wisdom, and to be explicit about where the current built-ins already diverge from it. Confidence varies a lot by topic: some of this rests on rigorous probability/simulation work, some is practitioner consensus with no hard numbers behind it — each subsection says which.

### Property value: landing frequency and rent-per-dollar

The best-supported claim in Monopoly strategy discourse, backed by an actual mechanism rather than opinion: **Jail is the single most-landed-on space on the board** (Markov-chain steady-state analysis puts it around 5.9% of turns), because Go To Jail, the three-doubles rule, and several cards all funnel players there — and the properties a roll or two past Jail inherit that traffic. Among purchasable spaces, Illinois Avenue is typically the single highest (~3.2%), and the Orange group as a whole is one of the most-landed-on on the board, followed by Red — both sit in the "just left Jail, rolled a 6-8" zone, which the dice-sum distribution makes disproportionately likely. Independent simulation studies of full games similarly find Orange/Pink/Light-Blue properties the most commonly *owned by eventual winners*, citing faster break-even (lower price and build cost, high traffic) as the reason.

The natural counterpart: **Boardwalk and Park Place (Dark Blue) are commonly cited as overrated** — highest face-value rent, but low landing frequency and the highest build cost on the board mean a slower payback than Orange or Red despite the more impressive numbers on the card. **Railroads are also consistently rated above Utilities** — flat, building-free rent that several sources describe as "a savings account," breaking even within roughly 8 landings, versus Utilities' dice-dependent and generally weaker return.

Sources: [Truman Collins' probability analysis](http://www.tkcs-collins.com/truman/monopoly/monopoly.shtml) (the primary rigorous source — simulation cross-checked against a Markov-chain model); [a Markov-chain writeup on Towards Data Science](https://towardsdatascience.com/oh-the-places-youll-go-in-monopoly-96abf70cdbd7/); [Jake Mitchell's 1,000-game simulation, also on Towards Data Science](https://towardsdatascience.com/a-data-driven-tactics-simulation-for-monopoly-864e7cffe508/); corroborated by [Dice and Deeds](https://diceanddeeds.com/how-to-win-monopoly-strategy/), [Tim Darling's guide](https://www.amnesta.net/monopoly/), and [Inverse's summary of Hannah Fry/Matt Parker's analysis](https://www.inverse.com/article/32781-win-monopoly-using-math).

### Building

Multiple sources converge on **3 houses being the rent-per-dollar sweet spot** — the jump from 2 to 3 houses is typically the largest single rent increase on a property (e.g. Orange: $200 to $550), and it recovers its own cost fastest; the 4th house and hotel have lower marginal return except on the highest-traffic properties. **Spreading houses evenly across a color group**, rather than maxing one property to a hotel while its group-mates stay bare, is recommended so any landing in the group hits something built. A more aggressive, explicitly named tactic is **house-supply denial**: building 4-house sets on a cheap monopoly early can lock up a large fraction of the game's fixed 32-house stock (see [game-rules.md](./game-rules.md#building-houses-and-hotels)), physically preventing opponents from developing expensive groups at all.

Sources: [Inverse](https://www.inverse.com/article/32781-win-monopoly-using-math) (citing Tim Darling's simulations and UK champion Natalie Fitzsimons), [Tim Darling's guide](https://www.amnesta.net/monopoly/) (specific rent-jump figures), [Dice and Deeds](https://diceanddeeds.com/how-to-win-monopoly-strategy/) (even distribution, supply denial).

### Jail

Sources consistently describe jail strategy as **phase-dependent, not fixed**: pay/use a card to leave immediately early in the game, since sitting out costs acquisition opportunities while the board is still open — then, once you hold built monopolies, deliberately *stay* in jail rather than paying to leave, since you keep collecting rent passively from opponents while avoiding the risk of landing on their built-up properties yourself. None of the built-in strategies implement quite this reversal: Buy Good's jail logic ("stay while holding few/no monopolies, pay once holding one worth collecting on" — see above) is the closest, but it's framed around *owning* a monopoly rather than specifically around *opponents' hotel risk*, which is the stated reasoning behind the real-world late-game "stay" advice.

Sources: [Inverse](https://www.inverse.com/article/32781-win-monopoly-using-math) (via Natalie Fitzsimons), [Dice and Deeds](https://diceanddeeds.com/how-to-win-monopoly-strategy/), [Tim Darling's guide](https://www.amnesta.net/monopoly/).

### Cash management

The consistent framing is **"cash in hand earns nothing while it sits there"** — hoarding is described as a losing habit relative to keeping money working in property and buildings, but *how much* reserve to hold is the softest number in this research: guides suggest reserves in the low hundreds of dollars, scaled to game phase (thinner early, larger once rents on the board get big), rather than any rigorously derived constant. Treat this as directional consensus, not a benchmark number — none of the sources back a specific reserve with simulation the way the property-value claims above are backed.

Sources: [Dice and Deeds](https://diceanddeeds.com/how-to-win-monopoly-strategy/), [The Econ Professor](https://theeconprofessor.com/using-monopoly-auctions-to-outbid-opponents-and-gain-property/).

### Auctions

Beyond bidding on properties you actually want, **denial bidding** — bidding up, or even winning, a property specifically to keep an opponent from completing a monopoly, independent of whether you want the property yourself — is named explicitly as a deliberate tactic, with a qualitative valuation frame of "your own value for it, plus set-completion value, plus denial value, capped by your cash reserve." None of the built-in strategies bid for denial; Buy All and Buy Good only bid up to their own affordability/heuristic ceiling for properties *they'd* want to own. This area has the weakest quantitative backing of the six — no source offers a rigorous bidding formula, only practitioner heuristics.

Source: [The Econ Professor](https://theeconprofessor.com/using-monopoly-auctions-to-outbid-opponents-and-gain-property/).

### Trading

The clearest connection back to this project: research on games without trading finds that only the *easiest-to-complete-by-chance* monopolies (small, 2-property groups like Brown or Dark Blue) tend to actually form, while larger, statistically better 3-property groups (Orange, Red, etc. — see above) usually end up split across players and never get built. That is the exact stalemate this engine's own Phase 1/2 batch runs already measured and documented independently (see [game-rules.md](./game-rules.md#bankruptcy)) — the external research corroborates rather than adds to that finding. It's a concrete, mechanism-backed reason to expect Phase 7's trading to matter a great deal in practice, not just add surface area.

**Buy All and Buy Good** (identical trading logic, differing only in every other decision) implement exactly this: `propose_monopoly_completing_trade` scans for a color group where the strategy owns all but one property and a single other player owns the rest, then proposes either a direct swap (if the strategy holds a "spare" property — one it doesn't otherwise need — that would complete a *different* group for that same counterparty) or a cash offer at a 1.5x premium over the missing property's list price. `decide_trade_response` accepts an incoming offer if it would complete a monopoly, or if it's a pure cash buyout paying more than the requested properties' list price. **Buy Bad and Buy None** never propose or accept a trade — consistent with their existing "does the least" character. `docs/game-rules.md#trading` has the full validation/execution mechanics.

### Where the built-in strategies diverge from this

Historical note (true through Phase 6, before Buy Shrewd): none of Buy All/Good/Bad/None weighted purchases or bids by landing frequency, implemented an opponents'-hotel-risk jail policy, or did house-supply-denial/auction-denial — all building and bidding was purely for the acting strategy's own benefit. **Buy Shrewd (above) now covers all four.** Two items remain open:

- **Buy All's $50 reserve** is thinner than the low-hundreds figure suggested online, though there's no rigorous benchmark to compare it against precisely.
- Buy Good/Buy All/Buy Shrewd's trading logic (`propose_monopoly_completing_trade`) only ever proposes a *direct* monopoly-completing swap; it doesn't negotiate combinations (e.g. two properties plus cash) or evaluate a trade that would help the counterparty more than itself but still be net-positive. A more sophisticated trade-evaluation heuristic is a reasonable further "custom strategy" (see below), not something the built-ins need to chase.

## Custom strategies

Anything implementing the `Strategy` trait can be used interchangeably with the built-ins — by the CLI (`--strategy` referencing a registered implementation or a scripted config), by the server for batch dispatch, and by the browser's strategy picker (which lists whatever strategies the running engine build has registered). As of Phase 7, the trait has no default method implementations — a custom strategy must implement all seven hooks, even a trivial one for a hook it doesn't care about (e.g. `decide_build` returning an empty `Vec`, or `decide_trade` returning `None`). Default implementations (e.g. mirroring Buy Good) are a reasonable future addition once a real custom strategy actually needs to override only one or two hooks; nothing built-in does.

Parameterized variants (e.g. "Buy Good with a $300 reserve" instead of $150) are expressed as config on top of a base strategy rather than as wholly new types — see `StrategyConfig` in [data-model.md](./data-model.md#playerconfig).
