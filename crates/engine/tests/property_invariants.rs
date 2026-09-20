//! Property-based invariants (`docs/testing-and-validation.md`) that must
//! hold for *any* valid `(RuleSet, players, seed)`, now that there's enough
//! rule interaction (houses, mortgages, cards, auctions, bankruptcy-to-player)
//! to make fuzzing worthwhile.
//!
//! `reconcile_final_cash` deliberately does **not** reuse
//! `monopoly_engine::stats`'s event reducer, even though the two are
//! structurally similar — the point of this test is independent
//! verification, so a bug shared between the production code and its check
//! would otherwise go undetected.

use monopoly_engine::board::{Board, BOARD_SIZE};
use monopoly_engine::state::{STARTING_HOTELS, STARTING_HOUSES};
use monopoly_engine::{
    CardEffect, Event, EventEnvelope, Game, IncomeTaxMode, JailAction, PlayerConfig, RuleSet,
};
use proptest::prelude::*;

const STRATEGY_IDS: [&str; 5] = ["buy_all", "buy_good", "buy_bad", "buy_none", "buy_shrewd"];

fn player_config() -> impl Strategy<Value = PlayerConfig> {
    proptest::sample::select(&STRATEGY_IDS[..]).prop_map(|s| PlayerConfig {
        name: s.to_string(),
        strategy: s.to_string(),
    })
}

fn players_config() -> impl Strategy<Value = Vec<PlayerConfig>> {
    proptest::collection::vec(player_config(), 2..=4)
}

/// `max_turns` is fixed and small so every generated game runs fast — this
/// is what the bounded-termination invariant below actually exercises, and
/// what keeps the whole suite quick despite fuzzing many games.
/// `free_parking_pot` is left off: its payout isn't independently
/// event-logged (see `monopoly_engine::stats`'s module doc comment), so
/// reconciling it here would mean mirroring internal bookkeeping rather than
/// replaying events — out of scope for this event-log-only check.
/// `double_go_salary` is left off for the same reason: `reconcile_final_cash`
/// below credits every `PassGo` at the flat `rules.go_salary`, and telling a
/// doubled landing-on-GO payment apart from a normal pass-through would mean
/// cross-referencing the preceding `Move` event rather than just replaying
/// `PassGo` in isolation — covered by a dedicated `game.rs` unit test
/// instead. `unlimited_houses` is left off as well:
/// `bank_house_and_hotel_supply_always_balances` below asserts the bank's
/// fixed 32/12 supply is always exactly conserved, which `unlimited_houses`
/// deliberately breaks by design (see `game.rs`'s `try_build`) — fuzzing it
/// here would fight the very invariant that test checks, rather than test
/// it; covered instead by dedicated `game.rs` unit tests. `trading_enabled`
/// *is* fuzzed - `reconcile_final_cash` below has its own `TradeExecuted`
/// arm, independent of `stats.rs`'s.
fn rule_set() -> impl Strategy<Value = RuleSet> {
    (
        500u32..3000,
        50u32..400,
        10u32..100,
        10u32..150,
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        0u8..3,
    )
        .prop_map(
            |(
                starting_cash,
                go_salary,
                jail_fine,
                luxury_tax,
                even_build_rule,
                auction_on_decline,
                trading_enabled,
                tax_mode,
            )| {
                let income_tax_mode = match tax_mode {
                    0 => IncomeTaxMode::Flat { amount: 200 },
                    1 => IncomeTaxMode::Percentage { rate: 0.1 },
                    _ => IncomeTaxMode::Choice { flat_amount: 200 },
                };
                RuleSet {
                    starting_cash,
                    go_salary,
                    jail_fine,
                    luxury_tax,
                    income_tax_mode,
                    even_build_rule,
                    auction_on_decline,
                    free_parking_pot: false,
                    max_turns: Some(300),
                    double_go_salary: false,
                    unlimited_houses: false,
                    trading_enabled,
                }
            },
        )
}

/// `payer` owes `amount`. Before failing, the engine tries to raise cash by
/// selling houses/mortgaging `payer`'s own properties (`raise_cash`) — so a
/// `Bankrupted` event for `payer` isn't necessarily the *very* next envelope,
/// it can follow a run of `payer`'s own `Mortgaged`/`HouseSold` events first.
/// This credits those itself, pays if and only if `payer` can then cover
/// `amount` (the way the engine decides it), and returns how many envelopes
/// after `start` it consumed so the caller's cursor can skip them. It stops
/// *before* any trailing `Bankrupted` envelope, which the caller applies
/// itself.
///
/// Deciding "was this paid?" by looking ahead for that `Bankrupted` envelope
/// would be wrong — `CardEffect::PayEachPlayer` charges the same player once
/// per recipient, so an earlier recipient's *successful* payment can sit in
/// the log directly before the charge that bankrupts them, with nothing in
/// between to separate the two raise-cash runs.
fn settle(
    cash: &mut [i64],
    board: &Board,
    events: &[EventEnvelope],
    start: usize,
    payer: usize,
    amount: u32,
    payee: Option<usize>,
) -> usize {
    let mut cursor = start;
    while cash[payer] < amount as i64 {
        let Some(env) = events.get(cursor) else { break };
        if env.player != payer {
            break;
        }
        match env.event {
            Event::Mortgaged { space } => {
                cash[payer] += (board.space(space).price().unwrap_or(0) / 2) as i64;
            }
            Event::HouseSold { space } => {
                cash[payer] += (board.space(space).house_cost().unwrap_or(0) / 2) as i64;
            }
            _ => break,
        }
        cursor += 1;
    }
    if cash[payer] >= amount as i64 {
        cash[payer] -= amount as i64;
        if let Some(payee) = payee {
            cash[payee] += amount as i64;
        }
    }
    cursor - start
}

/// Replays every cash-affecting event independently and returns the final
/// cash it implies for each player. Walks the log with an explicit cursor
/// (rather than a plain iteration) because `settle` above can consume more
/// than one envelope per debt. It can also consume *fewer* than the engine's
/// `raise_cash` emitted — it stops as soon as the debt is affordable, while
/// the engine applies every action the strategy returned — so leftover
/// raise-cash envelopes are handled bare below, with the identical credit.
fn reconcile_final_cash(
    board: &Board,
    rules: &RuleSet,
    n: usize,
    events: &[EventEnvelope],
) -> Vec<i64> {
    let mut cash = vec![rules.starting_cash as i64; n];
    let mut bankrupt = vec![false; n];
    let mut pending_offer_price: Option<u32> = None;

    let mut i = 0;
    while i < events.len() {
        let env = &events[i];
        let mut skip = 0;
        match &env.event {
            Event::PassGo => cash[env.player] += rules.go_salary as i64,
            Event::PropertyOffered { price, .. } => pending_offer_price = Some(*price),
            Event::PurchaseDecision { bought, .. } => {
                let price = pending_offer_price.take();
                if *bought {
                    cash[env.player] -= price.unwrap_or(0) as i64;
                }
            }
            Event::AuctionWon { player, amount, .. } => cash[*player] -= *amount as i64,
            Event::HouseBuilt { space } => {
                cash[env.player] -= board.space(*space).house_cost().unwrap_or(0) as i64;
            }
            Event::HouseSold { space } => {
                cash[env.player] += (board.space(*space).house_cost().unwrap_or(0) / 2) as i64;
            }
            Event::Mortgaged { space } => {
                cash[env.player] += (board.space(*space).price().unwrap_or(0) / 2) as i64;
            }
            Event::TaxPaid { amount, .. } => {
                skip = settle(&mut cash, board, events, i + 1, env.player, *amount, None);
            }
            Event::RentPaid { to, amount, .. } => {
                skip = settle(
                    &mut cash,
                    board,
                    events,
                    i + 1,
                    env.player,
                    *amount,
                    Some(*to),
                );
            }
            Event::JailDecision {
                action: JailAction::PayFine,
                ..
            } => {
                skip = settle(
                    &mut cash,
                    board,
                    events,
                    i + 1,
                    env.player,
                    rules.jail_fine,
                    None,
                );
            }
            Event::CardDrawn { effect, .. } => match effect {
                CardEffect::CollectFromBank(amount) => cash[env.player] += *amount as i64,
                CardEffect::PayBank(amount) => {
                    skip = settle(&mut cash, board, events, i + 1, env.player, *amount, None);
                }
                CardEffect::CollectFromEachPlayer(amount) => {
                    for (other, &is_bankrupt) in bankrupt.iter().enumerate() {
                        if other == env.player || is_bankrupt {
                            continue;
                        }
                        skip += settle(
                            &mut cash,
                            board,
                            events,
                            i + 1 + skip,
                            other,
                            *amount,
                            Some(env.player),
                        );
                    }
                }
                CardEffect::PayEachPlayer(amount) => {
                    for (other, &is_bankrupt) in bankrupt.iter().enumerate() {
                        if other == env.player || is_bankrupt {
                            continue;
                        }
                        // No explicit stop once the payer goes bankrupt
                        // part-way through the card — `settle`'s own
                        // affordability check refuses every remaining
                        // recipient, which is what the engine's `break` does.
                        skip += settle(
                            &mut cash,
                            board,
                            events,
                            i + 1 + skip,
                            env.player,
                            *amount,
                            Some(other),
                        );
                    }
                }
                CardEffect::PropertyRepairAssessment { .. }
                | CardEffect::AdvanceTo(_)
                | CardEffect::AdvanceToNearestRailroad
                | CardEffect::AdvanceToNearestUtility
                | CardEffect::GoBackThreeSpaces
                | CardEffect::GoToJail
                | CardEffect::GetOutOfJailFree => {}
            },
            Event::Bankrupted { payee } => {
                let remaining = cash[env.player].max(0);
                cash[env.player] = 0;
                if let Some(p) = payee {
                    cash[*p] += remaining;
                }
                bankrupt[env.player] = true;
            }
            Event::TradeExecuted {
                to,
                offered_cash,
                requested_cash,
                ..
            } => {
                cash[env.player] -= *offered_cash as i64;
                cash[*to] += *offered_cash as i64;
                cash[*to] -= *requested_cash as i64;
                cash[env.player] += *requested_cash as i64;
            }
            _ => {}
        }
        i += 1 + skip;
    }
    cash
}

/// A seeded companion to `cash_reconciles_against_the_full_event_log` below,
/// pinning the one scenario random generation is too sparse to find
/// reliably: this game draws "chairman of the board"
/// (`CardEffect::PayEachPlayer`), and the drawer pays two opponents
/// successfully before being bankrupted by the third — with the `Bankrupted`
/// envelope sitting immediately after the `CardDrawn` one, nothing in
/// between. Deciding whether a payment went through by peeking ahead for
/// that envelope dropped all three payments; only about one game in 600
/// hits it, so 48 proptest cases caught it roughly 7% of the time.
#[test]
fn a_bankruptcy_part_way_through_pay_each_player_still_pays_the_earlier_players() {
    let rules = RuleSet::default();
    let players: Vec<PlayerConfig> = ["buy_all", "buy_good", "buy_bad", "buy_none"]
        .iter()
        .map(|s| PlayerConfig {
            name: s.to_string(),
            strategy: s.to_string(),
        })
        .collect();
    let board = Board::standard();
    let mut game = Game::new(rules.clone(), &players, 4).unwrap();
    let result = game.run_to_completion();

    let drawn_then_bankrupt = result.events.windows(2).any(|w| {
        matches!(
            w[0].event,
            Event::CardDrawn {
                effect: CardEffect::PayEachPlayer(_),
                ..
            }
        ) && matches!(w[1].event, Event::Bankrupted { .. })
            && w[0].player == w[1].player
    });
    assert!(
        drawn_then_bankrupt,
        "seed 4 no longer reproduces the scenario this test exists for"
    );

    let reconciled = reconcile_final_cash(&board, &rules, players.len(), &result.events);
    for (i, player) in result.final_state.players.iter().enumerate() {
        assert_eq!(reconciled[i], player.cash, "player {i}");
    }
}

proptest! {
    // Higher than the other invariants below: the bug this caught during
    // Phase 3 review (a bankruptcy partway through PayEachPlayer/
    // CollectFromEachPlayer silently voiding an earlier, already-successful
    // payment) only manifests with 3+ players, that specific card drawn, and
    // a bankruptcy on a non-first recipient — measured at roughly 1 in 600
    // games, so a low case count gives a low chance of ever exercising it.
    // Games are cheap here (max_turns caps every one), so this can afford to
    // be generous.
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Codifies both the Phase 2 bankruptcy-cash-loss bug and the Phase 3
    /// bankruptcy-mid-multi-payment bug (see `a_bankruptcy_part_way_through_
    /// pay_each_player_still_pays_the_earlier_players` above for the pinned
    /// regression) as a standing invariant: independently replaying every
    /// cash-affecting event must land on exactly the engine's own final cash
    /// for every player.
    #[test]
    fn cash_reconciles_against_the_full_event_log(
        rules in rule_set(),
        players in players_config(),
        seed in any::<u64>(),
    ) {
        let board = Board::standard();
        let mut game = Game::new(rules.clone(), &players, seed).unwrap();
        let result = game.run_to_completion();
        let reconciled = reconcile_final_cash(&board, &rules, players.len(), &result.events);
        for (i, player) in result.final_state.players.iter().enumerate() {
            prop_assert_eq!(reconciled[i], player.cash, "player {}", i);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn bank_house_and_hotel_supply_always_balances(
        rules in rule_set(),
        players in players_config(),
        seed in any::<u64>(),
    ) {
        let mut game = Game::new(rules, &players, seed).unwrap();
        let result = game.run_to_completion();
        let state = &result.final_state;

        let houses_in_play: u32 = state.properties.iter().map(|p| if p.houses < 5 { p.houses as u32 } else { 0 }).sum();
        let hotels_in_play: u32 = state.properties.iter().filter(|p| p.houses == 5).count() as u32;

        prop_assert_eq!(state.bank_houses_remaining as u32 + houses_in_play, STARTING_HOUSES as u32);
        prop_assert_eq!(state.bank_hotels_remaining as u32 + hotels_in_play, STARTING_HOTELS as u32);
    }

    #[test]
    fn every_owned_property_belongs_to_a_valid_non_bankrupt_player(
        rules in rule_set(),
        players in players_config(),
        seed in any::<u64>(),
    ) {
        let n = players.len();
        let mut game = Game::new(rules, &players, seed).unwrap();
        let result = game.run_to_completion();
        let state = &result.final_state;

        for space in 0..BOARD_SIZE {
            if let Some(owner) = state.properties[space].owner {
                prop_assert!(owner < n, "space {space} owned by out-of-range player {owner}");
                prop_assert!(!state.players[owner].bankrupt, "space {space} still owned by bankrupt player {owner}");
            }
        }
    }

    #[test]
    fn a_set_max_turns_always_bounds_the_game(
        rules in rule_set(),
        players in players_config(),
        seed in any::<u64>(),
    ) {
        let max_turns = rules.max_turns.expect("rule_set() always sets max_turns");
        let mut game = Game::new(rules, &players, seed).unwrap();
        let result = game.run_to_completion();
        prop_assert!(result.turns <= max_turns);
    }
}
