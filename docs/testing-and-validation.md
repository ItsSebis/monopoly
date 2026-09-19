# Testing & Validation

"As close to the real game as possible" is a testable claim, not just a design intent — this doc defines how the `engine` crate is checked against real Monopoly rules.

## Unit tests: exact rule outcomes

One test per rule-fidelity edge case, each asserting an exact numeric or state outcome, not just "it doesn't crash":

- Rolling doubles three times in one turn sends the player to jail instead of moving on the third roll.
- Utility rent is exactly 4× the dice roll with one utility owned, 10× with both — never a flat amount.
- Railroad rent is exactly $25/$50/$100/$200 for 1/2/3/4 owned by the same player.
- Street rent doubles when the owner holds the full color group but has built no houses yet (the easy-to-get-wrong "monopoly bonus without building" case).
- A mortgaged property charges no rent, and unmortgaging costs exactly mortgage value + 10%.
- The even-build rule rejects a build that would put one property more than one house ahead of the least-built property in its group.
- Bank house/hotel supply is enforced: a build request is rejected once the relevant supply (32 houses / 12 hotels) is exhausted, even if the player can afford it.
- Bankruptcy-to-player transfers all remaining properties (with correct mortgage state) and any "Get Out of Jail Free" cards; bankruptcy-to-bank returns properties to the unowned pool.
- Each of the 32 Chance/Community Chest cards (16 per deck, official set) produces its documented effect exactly once verified individually, including the less common ones (move to nearest railroad/utility with rent-doubling, "pay each player $X", street repair assessments).

## Scenario/replay tests

A handful of hand-authored or recorded real-game turn sequences, encoded as fixed `(RuleSet, players, seed)` triples with an expected final outcome, run as regression tests. These catch interaction bugs between rules (e.g. a card effect combined with an in-progress auction) that isolated unit tests can miss. Because the engine is deterministic ([simulation-engine.md](./simulation-engine.md#determinism)), any scenario that ever produces a wrong or suspicious result can be pinned as a permanent regression test just by recording its seed.

## Property-based tests

Implemented in Phase 3 (`crates/engine/tests/property_invariants.rs`, `proptest`), covering every valid `(RuleSet, players, seed)` the generators produce (2-4 players, the four built-in strategies, randomized `RuleSet` fields, `max_turns` fixed and small so every generated game runs fast):

- **Cash-ledger reconciliation** (`cash_reconciles_against_the_full_event_log`, 2,000 cases — deliberately more than the others below, see why in the test's own comment): an *independent* reimplementation of the event-replay logic (not reusing `engine::stats`, so a bug shared between production code and its check can't hide) must reconstruct exactly the engine's own final cash for every player from the event log alone. This is the sharpest of the four — it caught two real bugs during Phase 3's own review: Phase 2's bankruptcy-cash-loss shape recurring in a new form, and a subtler one where a bankruptcy partway through a `PayEachPlayer`/`CollectFromEachPlayer` card (which charges the same player once per recipient) made a naive replay wrongly conclude an *earlier, already-successful* payment in the same card never happened. That second bug only manifests with 3+ players and a bankruptcy on a non-first recipient — measured at roughly 1 in 600 games — which is why this specific invariant runs far more cases than the others.
- **Bank house/hotel supply always balances**: `bank_houses_remaining` + houses in play, and `bank_hotels_remaining` + hotels in play, always equal the fixed starting stock (32/12) — supply can never leak or be double-counted.
- **Every owned property belongs to a valid, non-bankrupt player** — no-two-players-own-the-same-property is guaranteed by construction (one `owner: Option<usize>` per space), so this checks the part that isn't: a bankrupt player's properties are never left dangling in their name.
- **A set `max_turns` always bounds the game** (verified to bind in practice, not just in principle: roughly half the generated games hit the cap). Unbounded termination (a property-buying matchup eventually bankrupts down to one player without a cap) is deliberately *not* fuzzed here — Phase 1/2 already found and documented strategy mixes (e.g. Buy Good vs. Buy Good) that can legitimately run very long without trading, so an open-ended property test for this would be flaky by design. That case is covered instead by `buy_all_vs_buy_none_reliably_terminates_quickly`'s targeted seed loop (below), a matchup chosen specifically because one side's cash can only trend downward.

## Where this fits in the roadmap

Rule-fidelity unit tests are written alongside each rule as it's implemented (Phase 1 covers the baseline turn loop's tests; Phase 2 adds tests for houses/hotels/cards/auctions/jail as those land) rather than as a separate pass — see [roadmap.md](./roadmap.md). Property-based tests landed in Phase 3, once the full Phase 2 ruleset gave them real interaction complexity to catch bugs in. A dedicated scenario/replay regression file hasn't been split out yet, but the pattern already exists in miniature: `property_invariants.rs`'s `a_bankruptcy_part_way_through_pay_each_player_still_pays_the_earlier_players` pins the exact `(RuleSet, players, seed)` that exposed the bug above as a permanent regression test, per this doc's original idea of pinning any scenario that ever produces a wrong result.
