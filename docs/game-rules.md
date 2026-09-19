# Game Rules

This is the rules contract the `engine` crate must satisfy. It has two parts: the fixed baseline (standard Monopoly, US edition rules) and the configurable toggles exposed through `RuleSet` (see [data-model.md](./data-model.md#ruleset)). "As close to the real game as possible" means: baseline rules are implemented exactly, and every commonly-played house-rule variation is a config toggle rather than a hardcoded assumption.

## Board

Standard 40-space board:

- **GO** (space 0) — passing or landing collects the GO salary (default $200, configurable).
- **22 street properties** in 8 color groups (2–3 per group), each with a purchase price, a base rent, rent-with-monopoly (double base), and rent per house/hotel level (1–4 houses, then hotel).
- **4 railroads** — rent depends on how many of the 4 the same owner holds ($25/$50/$100/$200 for 1/2/3/4 owned).
- **2 utilities** (Electric Company, Water Works) — rent is a dice-roll multiplier: 4× the dice roll if the owner holds one utility, 10× if both.
- **3 Chance spaces**, **3 Community Chest spaces** — draw the top card of the respective deck, apply its effect, then return it to the bottom (decks are shuffled once per game using the game's RNG seed).
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
- **Leaving jail**, tried in this order once a player's turn begins in jail: (a) play a "Get Out of Jail Free" card if held, (b) pay the jail fine (default $50, configurable) if the strategy chooses to, (c) roll for doubles — success releases the player and uses that roll to move; failure keeps them jailed and ends the turn. A player gets up to 3 turns to roll doubles; if the third roll also fails, they must pay the fine immediately and move using that same roll — this cap is part of the fixed baseline, not a toggle.
- The choice between (a)/(b)/(c) when multiple are available is a strategy decision (`Strategy::decide_jail_action`).

## Building houses and hotels

- A player must own every property in a color group (a monopoly) before building on any property in that group.
- **Even-build rule** (toggle, default on): within a group, no property may have more than one more house than the least-built property in that group — houses must be built up evenly across the group.
- The bank has a finite supply: 32 houses and 12 hotels (fixed baseline, not configurable — this scarcity is part of what makes real Monopoly strategy interesting). A hotel replacement returns that property's 4 houses to the bank supply.
- Building/selling houses is a strategy decision (`Strategy::decide_build`), evaluated once per turn per eligible group after the player's own landing resolution completes (mirroring how real play typically happens between/around turns).

## Mortgaging

- A property can be mortgaged for half its purchase price when the owner needs cash; a mortgaged property earns no rent from opponents landing on it.
- Unmortgaging costs the mortgage value plus 10% interest.
- Buildings must be sold before mortgaging a property in a built-up group.
- Mortgage/unmortgage decisions are made by `Strategy::decide_mortgage`, checked whenever a player owes a payment it cannot otherwise cover.

## Bankruptcy

- If a player owes more than they can raise (via cash, mortgaging, and selling houses back to the bank at half price), they go bankrupt.
- Bankrupt **to another player** (e.g. unpayable rent): all remaining properties transfer to the creditor, along with any "Get Out of Jail Free" cards, at their current mortgage state.
- Bankrupt **to the bank** (e.g. unpayable tax): all properties return to the bank and become available for purchase/auction again.
- The bankrupt player is removed from the game; if only one player remains, that player wins.
- **Without houses** (Phase 1's scope), base rents are small enough relative to GO salary that two cash-accumulating strategies can occasionally out-earn each other indefinitely, with no bankruptcy ever occurring — confirmed empirically during Phase 1 implementation. This is expected, not a bug: house-building (Phase 2) multiplies rent far past salary income, which is what makes bankruptcy reliable in the real game.

## Income tax

Configurable mode (`RuleSet.income_tax_mode`):

- **Flat** — pay a fixed amount (default $200).
- **Percentage** — pay 10% of total net worth (cash + property purchase prices, undiscounted for mortgages/buildings).
- **Choice** — the strategy picks whichever is cheaper (or, for a custom strategy, whichever it prefers) at landing time; this is the official-rules default and matches how the physical game is actually played.

## Free parking

Configurable (`RuleSet.free_parking_pot`):

- **Off** (default, matches official rules) — landing on Free Parking has no effect; taxes and fees paid to the bank simply leave the game.
- **On** (common house rule) — all money paid to the bank (taxes, fines, mortgage interest) accumulates in a pot; landing on Free Parking collects the entire pot.

## Auctions

Configurable (`RuleSet.auction_on_decline`, default on, matching official rules):

- When a player declines to buy an unowned property they landed on, it goes to auction among all players (including the one who declined). Bidding starts at $1 with no upper limit tied to list price. Each strategy participates via `Strategy::decide_auction_bid` (see [player-strategies.md](./player-strategies.md)); a strategy may bid $0 to abstain. Highest bidder pays the bank and takes the property. If everyone abstains, the property remains unowned.
- When disabled, a declined property simply stays unowned and available for a future landing to trigger a fresh purchase offer.

## Trading

Not part of the baseline or Phases 1–6. Player-initiated trades are a [Phase 7](./roadmap.md#phase-7--advanced-strategies-stretch) extension; documented here so it's clear the omission through Phase 6 is deliberate, not an oversight.
