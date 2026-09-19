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

Invariants that must hold for *any* valid `(RuleSet, players, seed)`, checked over many randomly generated inputs (`proptest`):

- **Money conservation**: total money in the system (all players' cash + free parking pot, if enabled) only decreases via payments to the bank, and only increases via GO salary/card effects funded by the bank — it's never created or destroyed by a player-to-player transfer.
- **House/hotel supply never goes negative** and never exceeds the fixed bank stock.
- **No property is owned by two players at once**, and every owned property's owner is a valid, non-bankrupt player.
- **The engine always terminates**: with `max_turns` set, `step_turn()` calls are bounded; without it, a game with at least one property-buying strategy present bankrupts down to one player in finite time (checked probabilistically over many seeds rather than proven analytically).

## Where this fits in the roadmap

Rule-fidelity unit tests are written alongside each rule as it's implemented (Phase 1 covers the baseline turn loop's tests; Phase 2 adds tests for houses/hotels/cards/auctions/jail as those land) rather than as a separate pass — see [roadmap.md](./roadmap.md). Property-based tests and the scenario/replay regression suite are introduced once the full ruleset from Phase 2 is in place, since they're most valuable once there's real interaction complexity to catch bugs in.
