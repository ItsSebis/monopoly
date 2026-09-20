# Game Rules

This is the rules contract the `engine` crate must satisfy. It has two parts: the fixed baseline (standard Monopoly, US edition rules) and the configurable toggles exposed through `RuleSet` (see [data-model.md](./data-model.md#ruleset)). "As close to the real game as possible" means: baseline rules are implemented exactly, and every commonly-played house-rule variation is a config toggle rather than a hardcoded assumption.

## Board

Standard 40-space board:

- **GO** (space 0) — passing or landing collects the GO salary (default $200, configurable); see [Double GO salary](#double-go-salary) for the common house-rule variant.
- **22 street properties** in 8 color groups (2–3 per group), each with a purchase price, a base rent, rent-with-monopoly (double base), and rent per house/hotel level (1–4 houses, then hotel).
- **4 railroads** — rent depends on how many of the 4 the same owner holds ($25/$50/$100/$200 for 1/2/3/4 owned).
- **2 utilities** (Electric Company, Water Works) — rent is a dice-roll multiplier: 4× the dice roll if the owner holds one utility, 10× if both.
- **3 Chance spaces**, **3 Community Chest spaces** — draw the top card of the respective deck, apply its effect, then return it to the bottom (decks are shuffled once per game using the game's RNG seed). Both decks are the classic 16-card US-edition set; cards with an identical mechanical effect (many are just "collect/pay a flat amount" with different flavor text) share one effect type — see [simulation-engine.md](./simulation-engine.md#cards).
- **Income Tax** (space 4) — pay a tax on landing (see [Income tax](#income-tax) below for the configurable mode).
- **Luxury Tax** (space 38) — flat $75 (or configured value) on landing.
- **Jail / Just Visiting** (space 10) — visiting has no effect; see [Jail](#jail) for arriving *in* jail.
- **Go To Jail** (space 30) — landing here sends the player directly to Jail (no salary for passing GO on this move).
- **Free Parking** (space 20) — see [Free parking](#free-parking) for the configurable pot rule.

## Turn sequence

1. If in jail, resolve the jail decision first (see [Jail](#jail)); this may end the turn without moving.
2. Roll two dice. Rolling doubles three times in a row sends the player directly to jail instead of moving the third roll.
3. Move the player forward by the roll total, collecting GO salary if passing or landing on GO.
4. Resolve the landed-on space (purchase offer, rent, tax, card draw, go-to-jail, free parking).
5. If doubles were rolled (and the player isn't now in jail), they roll again — back to step 2 — otherwise the turn passes to the next player.
6. A bankrupt player is removed from turn order; the game ends when one player remains (or, in analysis contexts, when a configured max-turn cap is hit — see [data-model.md](./data-model.md#ruleset)).

## Landing resolution

- **Unowned property/railroad/utility** — the current player's strategy is asked whether to buy at list price (`Strategy::decide_purchase`). If declined and the auction rule is enabled, the property goes to auction among all players; if the auction rule is disabled, it simply remains unowned. See [player-strategies.md](./player-strategies.md).
- **Owned by another, unmortgaged** — pay rent immediately: base/monopoly/house-scaled rent for streets, holdings-scaled rent for railroads, dice-multiplier rent for utilities. Rent is never charged while the owner is in jail (rent still applies as normal — being in jail only affects the owner's own movement, not their properties' rent).
- **Owned by another, mortgaged** — no rent due.
- **Owned by the current player** — no effect.

## Jail

- **Entering jail**: landing on "Go To Jail", drawing a "Go to Jail" card, or rolling doubles three times in one turn. Entering jail always ends movement for that turn immediately.
- **Leaving jail**, tried in this order once a player's turn begins in jail: (a) a held "Get Out of Jail Free" card is always played automatically — it's never worse than the alternatives, so this isn't a `Strategy` decision at all; (b) otherwise, pay the jail fine (default $50, configurable) if the strategy chooses to, or (c) roll for doubles — success releases the player and uses that roll to move; failure keeps them jailed and ends the turn. A player gets up to 3 turns to roll doubles; if the third roll also fails, they must pay the fine immediately and move using that same roll — this cap is part of the fixed baseline, not a toggle.
- The choice between (b)/(c) is a strategy decision (`Strategy::decide_jail_action`), only asked when no card is held.

## Building houses and hotels

- A player must own every property in a color group (a monopoly) before building on any property in that group.
- **Even-build rule** (toggle, default on): within a group, no property may have more than one more house than the least-built property in that group — houses must be built up evenly across the group.
- The bank has a finite supply: 32 houses and 12 hotels (fixed baseline unless **Unlimited houses** below is enabled). A hotel replacement returns that property's 4 houses to the bank supply.
- Building/selling houses is a strategy decision (`Strategy::decide_build`), called once at the end of the player's own turn (after all their movement/landing resolution for that turn), returning a batch of actions across any number of their monopolies at once — mirroring how real play typically happens between/around turns, as a single "building phase" rather than a separate decision per group.

### Unlimited houses

Configurable (`RuleSet.unlimited_houses`, default off, matching official rules) — a common house rule for groups that run out of physical pieces:

- **Off** (default) — the bank's 32-house/12-hotel supply is a hard cap; building is refused once it's exhausted.
- **On** — the supply cap is ignored entirely; a monopoly can be built up to hotels regardless of what's "left in the box." The bank's supply counters simply stop moving while this is on (there's nothing meaningful left for them to track), rather than being allowed to under/overflow.

## Mortgaging

- A property can be mortgaged for half its purchase price when the owner needs cash; a mortgaged property earns no rent from opponents landing on it.
- Unmortgaging costs the mortgage value plus 10% interest. **Not modeled**: no built-in strategy ever chooses to unmortgage (see [player-strategies.md](./player-strategies.md)) — a mortgaged property stays mortgaged for the rest of the game. Voluntary unmortgaging is a candidate for a later phase, not a Phase 2 gap in fidelity for how these strategies actually play.
- Buildings must be sold before mortgaging a property in a built-up group.
- Mortgaging (and selling houses to raise cash) is a strategy decision (`Strategy::decide_mortgage`), asked whenever a player owes a payment it cannot otherwise cover.

## Bankruptcy

- If a player owes more than they can raise (via cash, mortgaging, and selling houses back to the bank at half price), they go bankrupt.
- Bankrupt **to another player** (e.g. unpayable rent): all remaining properties transfer to the creditor, along with any "Get Out of Jail Free" cards, at their current mortgage state.
- Bankrupt **to the bank** (e.g. unpayable tax): all properties return to the bank and become available for purchase/auction again.
- The bankrupt player is removed from the game; if only one player remains, that player wins.
- **Before Phase 2 added houses**, base rents were small enough relative to GO salary that two cash-accumulating strategies could occasionally out-earn each other indefinitely, with no bankruptcy ever occurring — confirmed empirically during Phase 1. House-building rent (up to $2000 on Boardwalk with a hotel, vs. a $200 GO salary) makes bankruptcy reliable *once someone forms a monopoly and builds*. Without trading (still absent — see [Trading](#trading)), whether anyone ever completes a monopoly at all depends on the initial ownership split from first-landing purchases and the handful of auctions/bankruptcies that follow, and 2-3 property groups often end up split across players by chance. Measured with the built-in strategies (`examples/four_player.toml`): a majority of games still fail to produce a winner within the internal safety cap, most often because the only monopolies that formed were held by a strategy that doesn't build. This is a known, structural limitation of no-trading Monopoly, not a bug — Phase 7's trading is what actually fixes it.

## Income tax

Configurable mode (`RuleSet.income_tax_mode`):

- **Flat** — pay a fixed amount (default $200).
- **Percentage** — pay 10% of total net worth (cash + property purchase prices, undiscounted for mortgages/buildings).
- **Choice** — the strategy picks whichever is cheaper (or, for a custom strategy, whichever it prefers) at landing time; this is the official-rules default and matches how the physical game is actually played.

## Free parking

Configurable (`RuleSet.free_parking_pot`):

- **Off** (default, matches official rules) — landing on Free Parking has no effect; taxes and fees paid to the bank simply leave the game.
- **On** (common house rule) — all money paid to the bank (taxes, fines, mortgage interest) accumulates in a pot; landing on Free Parking collects the entire pot.

## Double GO salary

Configurable (`RuleSet.double_go_salary`, default off, matching official rules) — the other most commonly cited GO-related house rule, alongside Free Parking's pot:

- **Off** (default) — a flat GO salary regardless of whether a move passes GO or lands exactly on it.
- **On** — landing **exactly** on GO pays double the normal salary; merely passing it on the way to another space still pays the normal (single) amount.

## Auctions

Configurable (`RuleSet.auction_on_decline`, default on, matching official rules):

- When a player declines to buy an unowned property they landed on (including via a card that advances onto one), it goes to auction among all non-bankrupt players (including the one who declined). Auctions are a single **sealed-bid** round rather than live ascending bidding: every player calls `Strategy::decide_auction_bid` once, with no visibility into anyone else's bid (see [player-strategies.md](./player-strategies.md)); a strategy may bid nothing to abstain. The highest bid wins, paying the *second*-highest bid (or $1 with only one bidder) — approximating how a live auction actually settles (bidding stops just past the runner-up's ceiling) without simulating rounds. If everyone abstains, the property remains unowned.
- When disabled, a declined property simply stays unowned and available for a future landing to trigger a fresh purchase offer.

## Trading

Configurable (`RuleSet.trading_enabled`, default off — matching every other optional rule here, and giving the roadmap's Phase 7 demo an exact same-strategy-code before/after lever). Added in Phase 7; not part of the baseline or Phases 1–6.

- When enabled, once at the end of `player`'s own turn (the same timing as the building phase), `Strategy::decide_trade` is called and may return a single proposed `TradeOffer`: properties and/or cash `player` would give up, and properties and/or cash they want back from a named counterparty.
- **Validation** happens before the counterparty is even asked: every offered/requested property must actually be owned by the expected side, unmortgaged, and undeveloped (`houses == 0` — a built-up property must be sold down first, same precedent as mortgaging), the counterparty must be a different, non-bankrupt player, and both sides must be able to afford the cash side. An invalid proposal is silently dropped, matching how every other `Strategy` action here is validated rather than trusted.
- A valid proposal is offered to the counterparty via `Strategy::decide_trade_response`. **This is a deliberate addition beyond the trait's original one-hook sketch** ([simulation-engine.md](./simulation-engine.md)): a trade means nothing without the other side's consent — the same reasoning that already makes `decide_auction_bid` ask every player, not just the current one. Real Monopoly trading is a back-and-forth negotiation; this models it as a single sealed accept/reject instead (no counter-offers), the same simplification auctions already make for the same reason — see [player-strategies.md](./player-strategies.md) for how the built-in strategies use this.
- An accepted trade is applied atomically: both properties and both cash amounts move in one step, or nothing does.
- Unlike the free-parking pot (see above), a trade's `TradeExecuted`/`TradeDeclined` event carries everything that moved, so it's fully replayable — batch/single-run statistics reconstruct it exactly, with no analogous gap.
