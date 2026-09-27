# Phase 8 strategy search results

A prior 2000-game tournament (`monopoly tournament --games 2000 --seed 1`, default rules) found Buy All (8.8% win rate) beating Buy Shrewd (5.9%) head-to-head (59.9%/40.1%), despite Buy Shrewd being the most behaviorally sophisticated built-in. Rather than guess at a fix, Phase 8 built `Configurable` — a strategy expressed entirely as data (`crates/engine/src/strategies/configurable.rs`) along 5 axes (`Valuation`, `JailPolicy`, `BuildPolicy`, `AuctionPolicy`, `TradePolicy`, plus a cash `reserve`) — and empirically searched it with a `monopoly sweep` CLI subcommand (`crates/cli/src/sweep.rs`). This document is that search's actual output, reproducible via `cargo run -p monopoly-cli --release -- sweep --seed <n> --out-dir sweep_results` (the raw CSVs/`summary.md` aren't checked in — they're `.gitignore`d as regenerable output — but every number quoted below was read directly off a real run, not invented).

## Method

1. **Stage 1** — every `JailPolicy` x `BuildPolicy{stop_before_hotel}` x `AuctionPolicy` x `TradePolicy` combination (3x2x2x2 = 24), each fixed at `Valuation::Weighted, reserve: 100`, seated once against the fixed 5-strategy panel (`buy_all, buy_good, buy_bad, buy_none, buy_shrewd`), rotated through all 6 seats (200 games per rotation, 1200 total), pooling the candidate's win rate across rotations.
2. This ran across **4 ruleset environments** (`examples/sweep_baseline.toml`, `sweep_trading.toml`, `sweep_house_rules.toml`, `sweep_maximal.toml` — trading and the three house rules toggled independently) so the answer isn't an artifact of one rule mix.
3. Each combo is ranked within each ruleset by **competition ranking** (ties share a rank; the next distinct value's rank skips accordingly) — plain ordinal (list-order) ranking would silently break ties in favor of whichever combo happened to come first in `stage_one_combos()`'s iteration order, and ties are common here (several axis values produce identical games on shared seeds, e.g. `JailPolicy::HotelRisk` and `PayIfAffordable` agree whenever no opponent has built a monopoly yet).
4. **Refinement** — each ruleset's own top stage-1 combo was re-run at 5 reserve values (`$50/$100/$150/$200/$300`) and a `Valuation::RentToPrice` variant, again by competition rank within the ruleset.
5. **Overall winner** — the combo with the best *mean rank* across all 4 rulesets' stage-1 rankings (the one candidate set scored identically in every ruleset), then its reserve/valuation refined once more by mean rank *within each ruleset* (not a single count pooled across all 4 — see the caveat below on why).
6. **Multi-seed check** — steps 1-5 were repeated at `--seed 1` through `--seed 5` before hardcoding anything, specifically to catch an axis choice that only looked like a winner because of one seed's particular games (see below — this mattered).

## Robust vs. inconclusive axes (the multi-seed check)

Running the full search at 5 different base seeds gave 5 independent "overall winners." Comparing them:

| seed | jail | auction | build (stop\_before\_hotel) | trade | reserve | valuation | mean-rank margin over runner-up |
|---|---|---|---|---|---|---|---|
| 1 | PayIfAffordable | ValuationCapped | false | MonopolyCompleting | 50 | Weighted | 1.00 (1.75 vs. 2.75) |
| 2 | HotelRisk | ValuationCappedWithDenial | false | MonopolyCompleting | 50 | Weighted | 0.50 (2.25 vs. 2.75) |
| 3 | HotelRisk | ValuationCappedWithDenial | false | MonopolyCompleting | 50 | Weighted | 0.50 (1.00 vs. 1.50) |
| 4 | Patient | ValuationCapped | false | MonopolyCompleting | 50 | Weighted | 0.25 (3.50 vs. 3.75) |
| 5 | HotelRisk | ValuationCappedWithDenial | false | MonopolyCompleting | 50 | Weighted | 1.00 (1.00 vs. 2.00) |

Two very different conclusions come out of this:

- **`build.stop_before_hotel: false`, `trade: MonopolyCompleting`, `reserve: 50`, and `valuation: Weighted` are solid.** Every one of the 5 seeds agreed on all four, and the reserve/valuation refinement pass was *monotonic* in every single seed — win rate strictly decreases from $50 up through $300, and `Weighted` beat `RentToPrice` at the same reserve every time (see each seed's refinement table; e.g. seed 1: $50 → 10.7% mean win rate, $100 → 10.4%, $150 → 10.0%, $200 → 9.8%, $300 → 9.0%, RentToPrice-at-$100 → 10.2%, worse than Weighted-at-$100). This isn't a coin flip that happened to land the same way 5 times — it's the same clean, monotonic curve in every seed. Reserve $50 also won regardless of which jail/auction combo was actually being refined (seeds 1 and 4 refined a *different* combo than seeds 2/3/5, and $50 still won cleanly in every case) — evidence the reserve preference doesn't interact with the noisy axes below.
- **`jail` and `auction` are not solid.** The winning value for each flips across seeds (`jail`: PayIfAffordable once, HotelRisk 3 times, Patient once; `auction`: ValuationCapped twice, ValuationCappedWithDenial 3 times), and the margin between the winner and the runner-up (0.25-1.00 mean-rank-places out of 24 combos ranked across 4 rulesets) is small enough to be within noise rather than a clear signal.

**Tiebreak used**: `JailPolicy::HotelRisk` and `AuctionPolicy::ValuationCappedWithDenial` — Buy Shrewd's own choice for both axes — tied for the win most often (3 of 5 seeds, and it was the *exact same* overall-winning combo in all 3, not just an axis-by-axis majority). This is reported honestly as "the most common tie," not "the proven best" — a different seed, or more seeds, could plausibly have picked `PayIfAffordable`/`ValuationCapped` instead (seed 1's actual winner).

## The winning configuration (`buy_optimal`)

```
ConfigurableParams {
    valuation: Valuation::Weighted,
    jail: JailPolicy::HotelRisk,
    build: BuildPolicy { stop_before_hotel: false },
    auction: AuctionPolicy::ValuationCappedWithDenial,
    trade: TradePolicy::MonopolyCompleting,
    reserve: 50,
}
```

In practice: *Buy Shrewd's jail policy (opponent-hotel-risk) and auction policy (denial bidding) and landing-frequency-weighted valuation, but building all the way to a hotel instead of stopping at 4 for house-supply denial, and Buy All's thinner $50 reserve instead of Buy Shrewd's $100.* Real per-ruleset win rates for this exact configuration, averaged across the same 5 seeds (1200 games/matchup each):

| ruleset | mean win rate across 5 seeds | per-seed range |
|---|---|---|
| sweep_baseline (no trading, no house rules) | 2.3% | 1.4% - 2.9% |
| sweep_trading (trading on) | 20.2% | 19.5% - 22.3% |
| sweep_house_rules (house rules on, no trading) | 1.6% | 1.0% - 2.0% |
| sweep_maximal (trading + all house rules) | 15.0% | 14.1% - 16.5% |

## Honest caveats

- **`jail` and `auction` are tie-broken, not proven.** See the multi-seed table above — don't read `buy_optimal`'s `HotelRisk`/`ValuationCappedWithDenial` choice as "the empirically best jail/auction policy against this panel." It's the most frequent winner across 5 seeds, with small margins over the runner-up in every seed it *did* win. A future re-run at more seeds could plausibly flip this.
- **Absolute win rates in the two no-trading rulesets (`sweep_baseline`, `sweep_house_rules`) are far below the naive 1-in-6 (16.7%) figure for a 6-player game — around 1-3% across every seed, winner included.** This is *not* the winning combo playing badly: it's that most 6-player games without trading never produce a winner at all within `max_turns` (this matches the project's own Phase 7 finding that trading roughly triples the game-completion rate — see `docs/player-strategies.md`'s Trading section and `docs/roadmap.md`'s Phase 7 entry — and 6 players spreads property ownership even thinner than the 4-player mix that finding was measured on). A supplementary check (`monopoly batch` on `buy_optimal`'s exact config seated against the same 5-strategy panel under `sweep_baseline.toml`, 1200 games, seed 1) found only about 13% of games produce *any* winner, and `buy_optimal` takes roughly **28% of those completed games** (versus a 16.7% share if it had no edge at all) — meaningfully ahead of chance once the game actually resolves, even though its raw, unconditional win rate looks tiny. This is why the refinement step ranks *within* each ruleset rather than pooling raw win counts across all 4 (`sweep.rs`'s `refine_by_rank`): a pooled comparison would be dominated entirely by whichever ruleset happens to finish games most often, rather than reflecting genuine relative strength.
- **`sweep_trading` clears the 16.7% bar clearly (20.2% mean); `sweep_maximal` sits just below it (15.0% mean, ranging up to 16.5% depending on seed)** — both are directly comparable win rates in environments where games routinely finish, and both are in the same range as chance-or-better for a 6-seat game, though `sweep_maximal` isn't unambiguously above it on every seed.
- **`Configurable` doesn't express every built-in's behavior** (Buy Bad's inverse-overbid auction logic and Buy None's total abstention have no analog on these axes) — by design, per the Phase 8 spec; the search only ever needed to beat the panel, not reproduce every built-in exactly. It also does *not* generally reproduce Buy All (see `configurable.rs`'s module doc comment and tests) — Buy All has no valuation threshold at all, while every `Configurable` decision is gated by `RATIO_THRESHOLD`.

## Reproducing this

```
cargo run -p monopoly-cli --release -- sweep --seed 1 --out-dir sweep_results
```

Takes about 10 seconds per seed on an 8-core machine and writes `sweep_results/{sweep_baseline,sweep_trading,sweep_house_rules,sweep_maximal}.csv` (every stage-1 combo plus refinement variants, ranked) and `sweep_results/summary.md`. Re-run at `--seed 2` through `5` (or further) to reproduce the multi-seed table above.
