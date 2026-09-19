//! Pure, event-log-driven statistics (`docs/analysis-and-metrics.md`). Takes
//! a finished `GameResult` and replays its event log to reconstruct
//! everything needed for analysis — no dependency on `Game` internals, so a
//! single run and a batch run (`batch.rs`) share this one aggregation step,
//! per `docs/simulation-engine.md`'s "one event format, three consumers"
//! design.
//!
//! Every cash-affecting event already carries the amount that was *supposed*
//! to move; the one subtlety is that a payment immediately followed by that
//! payer's `Bankrupted` event never fully went through (see `Event::RentPaid`
//! and `Event::Bankrupted`'s doc comments) — `apply_debt` below is the single
//! place that accounts for that.

use serde::Serialize;

use crate::board::{Board, ColorGroup, SpaceKind, BOARD_SIZE};
use crate::cards::CardEffect;
use crate::config::PlayerConfig;
use crate::events::{Event, EventEnvelope};
use crate::game::GameResult;
use crate::rules::RuleSet;
use crate::strategy::JailAction;

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct CashFlowBreakdown {
    pub rent_paid: i64,
    pub rent_received: i64,
    pub tax_paid: i64,
    /// Net of every Chance/Community Chest cash effect (positive = net
    /// receipts). The two "pay/collect from every other player" cards are
    /// folded in at their face value even on the rare turn one of those
    /// per-player payments is cut short by a bankruptcy — a cash-flow
    /// *breakdown* is a display aggregate, not the exact-cash invariant
    /// (see `crates/engine/tests/property_invariants.rs` for that).
    pub card_net: i64,
    pub go_salary_collected: i64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PropertyAcquired {
    pub space: usize,
    pub owner: usize,
    pub turn: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct MonopolyCompleted {
    pub group: ColorGroup,
    pub owner: usize,
    pub turn: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct BankruptcyRecord {
    pub player: usize,
    pub turn: u32,
    pub payee: Option<usize>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PropertyRoi {
    pub space: usize,
    /// The property's owner at game end, if any — what `roi_by_strategy`
    /// (`batch.rs`) groups by. A property that changed hands mid-game (only
    /// possible via bankruptcy-to-player) has its whole game's rent
    /// attributed here, a documented simplification.
    pub owner: Option<usize>,
    pub rent_collected: u32,
    pub cost_basis: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerGameStats {
    pub winner: Option<usize>,
    pub turns: u32,
    /// One row per turn boundary crossed, one entry per player: cash on hand
    /// plus property price plus building value (docs/analysis-and-metrics.md
    /// "Net worth over time").
    pub net_worth_by_turn: Vec<Vec<u32>>,
    pub cash_flow: Vec<CashFlowBreakdown>,
    pub property_timeline: Vec<PropertyAcquired>,
    pub monopolies_completed: Vec<MonopolyCompleted>,
    pub bankruptcies: Vec<BankruptcyRecord>,
    /// Indexed by `total - 2` for totals 2..=12.
    pub dice_roll_counts: [u64; 11],
    /// One entry per board space (`BOARD_SIZE` long) — a `Vec` rather than a
    /// fixed array because `serde`'s array impl only covers sizes up to 32
    /// (see `state.rs`'s `PropertyState` vec for the same reason).
    pub landing_counts: Vec<u64>,
    pub property_roi: Vec<PropertyRoi>,
}

/// The running cash/building state `compute_stats` reconstructs by replaying
/// events — bundled into one type so `apply_debt` (which needs all three)
/// doesn't have to take them as separate parameters.
struct Ledger<'a> {
    cash: &'a mut [i64],
    houses: &'a mut [u8],
    board: &'a Board,
}

impl Ledger<'_> {
    /// `payer` owes `amount`. Before failing, the engine tries to raise cash
    /// by selling houses/mortgaging `payer`'s own properties (`raise_cash`)
    /// — so a `Bankrupted` event for `payer` isn't necessarily the very next
    /// envelope, it can follow a run of `payer`'s own `Mortgaged`/`HouseSold`
    /// events first. This applies those events' cash and house-count effects
    /// itself and reports how many envelopes after `start` it consumed, so
    /// the caller's cursor can skip straight past them (stopping just
    /// *before* a trailing `Bankrupted` envelope, which the caller's own
    /// `Event::Bankrupted` handling applies uniformly regardless of which
    /// debt triggered it). If no bankruptcy follows the raise-cash run, the
    /// debt is paid in full here; otherwise this leaves `payer`'s cash
    /// exactly as the raise-cash run left it, for the caller to zero and
    /// hand off.
    fn apply_debt(
        &mut self,
        events: &[EventEnvelope],
        start: usize,
        payer: usize,
        amount: u32,
        payee: Option<usize>,
    ) -> usize {
        let mut peek = start;
        while let Some(env) = events.get(peek) {
            if env.player != payer {
                break;
            }
            match &env.event {
                Event::Mortgaged { space } => {
                    self.cash[payer] += (self.board.space(*space).price().unwrap_or(0) / 2) as i64;
                }
                Event::HouseSold { space } => {
                    self.cash[payer] +=
                        (self.board.space(*space).house_cost().unwrap_or(0) / 2) as i64;
                    self.houses[*space] = if self.houses[*space] == 5 {
                        4
                    } else {
                        self.houses[*space].saturating_sub(1)
                    };
                }
                _ => break,
            }
            peek += 1;
        }
        let consumed = peek - start;
        let bankruptcy_follows = matches!(
            events.get(peek),
            Some(env) if env.player == payer && matches!(env.event, Event::Bankrupted { .. })
        );
        if !bankruptcy_follows {
            self.cash[payer] -= amount as i64;
            if let Some(p) = payee {
                self.cash[p] += amount as i64;
            }
        }
        consumed
    }
}

fn net_worth_snapshot(
    board: &Board,
    n: usize,
    cash: &[i64],
    owner: &[Option<usize>],
    houses: &[u8],
) -> Vec<u32> {
    (0..n)
        .map(|p| {
            let base = cash[p].max(0) as u32;
            let property_value: u32 = (0..BOARD_SIZE)
                .filter(|&s| owner[s] == Some(p))
                .map(|s| {
                    let price = board.space(s).price().unwrap_or(0);
                    let house_cost = board.space(s).house_cost().unwrap_or(0);
                    let increments = if houses[s] == 5 { 5 } else { houses[s] as u32 };
                    price + house_cost * increments
                })
                .sum();
            base + property_value
        })
        .collect()
}

fn group_of(board: &Board, space: usize) -> Option<ColorGroup> {
    match board.space(space) {
        SpaceKind::Street { group, .. } => Some(group),
        _ => None,
    }
}

fn check_monopoly(
    board: &Board,
    owner: &[Option<usize>],
    space: usize,
    new_owner: usize,
    turn: u32,
    out: &mut Vec<MonopolyCompleted>,
) {
    let Some(group) = group_of(board, space) else {
        return;
    };
    if board
        .group_members(group)
        .all(|s| owner[s] == Some(new_owner))
    {
        out.push(MonopolyCompleted {
            group,
            owner: new_owner,
            turn,
        });
    }
}

/// Replays `result`'s event log once, reconstructing per-turn net worth,
/// cash-flow categories, the property/monopoly timeline, bankruptcies, dice
/// and landing distributions, and per-property ROI.
pub fn compute_stats(
    board: &Board,
    players: &[PlayerConfig],
    rules: &RuleSet,
    result: &GameResult,
) -> PerGameStats {
    let n = players.len();
    let mut cash: Vec<i64> = vec![rules.starting_cash as i64; n];
    let mut bankrupt = vec![false; n];
    let mut owner: Vec<Option<usize>> = vec![None; BOARD_SIZE];
    let mut houses: Vec<u8> = vec![0; BOARD_SIZE];
    let mut cash_flow = vec![CashFlowBreakdown::default(); n];
    let mut rent_collected = [0u32; BOARD_SIZE];
    let mut property_timeline = Vec::new();
    let mut monopolies_completed = Vec::new();
    let mut bankruptcies = Vec::new();
    let mut dice_roll_counts = [0u64; 11];
    let mut landing_counts = vec![0u64; BOARD_SIZE];
    let mut net_worth_by_turn = Vec::new();
    let mut pending_offer_price: Option<u32> = None;
    let mut current_turn = 0u32;

    // An explicit cursor rather than a plain iteration: `apply_debt` can
    // consume more than one envelope per debt (a raise-cash run of
    // `Mortgaged`/`HouseSold` events), and the cursor skips straight past
    // whatever it consumed.
    let events = &result.events;
    let mut i = 0;
    while i < events.len() {
        let env = &events[i];
        if env.turn != current_turn {
            net_worth_by_turn.push(net_worth_snapshot(board, n, &cash, &owner, &houses));
            current_turn = env.turn;
        }
        let mut skip = 0;
        match &env.event {
            Event::RollDice { dice } => dice_roll_counts[(dice.0 + dice.1) as usize - 2] += 1,
            Event::Move { to, .. } => landing_counts[*to] += 1,
            Event::PassGo => {
                cash[env.player] += rules.go_salary as i64;
                cash_flow[env.player].go_salary_collected += rules.go_salary as i64;
            }
            Event::PropertyOffered { price, .. } => pending_offer_price = Some(*price),
            Event::PurchaseDecision { space, bought } => {
                let price = pending_offer_price.take();
                if *bought {
                    let price = price.unwrap_or_else(|| board.space(*space).price().unwrap_or(0));
                    cash[env.player] -= price as i64;
                    owner[*space] = Some(env.player);
                    property_timeline.push(PropertyAcquired {
                        space: *space,
                        owner: env.player,
                        turn: env.turn,
                    });
                    check_monopoly(
                        board,
                        &owner,
                        *space,
                        env.player,
                        env.turn,
                        &mut monopolies_completed,
                    );
                }
            }
            Event::AuctionWon {
                player,
                space,
                amount,
            } => {
                cash[*player] -= *amount as i64;
                owner[*space] = Some(*player);
                property_timeline.push(PropertyAcquired {
                    space: *space,
                    owner: *player,
                    turn: env.turn,
                });
                check_monopoly(
                    board,
                    &owner,
                    *space,
                    *player,
                    env.turn,
                    &mut monopolies_completed,
                );
            }
            Event::HouseBuilt { space } => {
                let cost = board.space(*space).house_cost().unwrap_or(0);
                cash[env.player] -= cost as i64;
                houses[*space] = if houses[*space] == 4 {
                    5
                } else {
                    houses[*space] + 1
                };
            }
            // A bare `HouseSold`/`Mortgaged` here (not consumed by
            // `apply_debt` below) is a voluntary sale via `decide_build`
            // rather than a forced raise-cash one via `decide_mortgage` —
            // still a plain cash/house-count effect either way.
            Event::HouseSold { space } => {
                let cost = board.space(*space).house_cost().unwrap_or(0);
                cash[env.player] += (cost / 2) as i64;
                houses[*space] = if houses[*space] == 5 {
                    4
                } else {
                    houses[*space].saturating_sub(1)
                };
            }
            Event::Mortgaged { space } => {
                let price = board.space(*space).price().unwrap_or(0);
                cash[env.player] += (price / 2) as i64;
            }
            Event::TaxPaid { amount, .. } => {
                skip = Ledger {
                    cash: &mut cash,
                    houses: &mut houses,
                    board,
                }
                .apply_debt(events, i + 1, env.player, *amount, None);
                cash_flow[env.player].tax_paid += *amount as i64;
            }
            Event::RentPaid { to, amount, space } => {
                skip = Ledger {
                    cash: &mut cash,
                    houses: &mut houses,
                    board,
                }
                .apply_debt(events, i + 1, env.player, *amount, Some(*to));
                cash_flow[env.player].rent_paid += *amount as i64;
                cash_flow[*to].rent_received += *amount as i64;
                rent_collected[*space] += *amount;
            }
            Event::JailDecision {
                action: JailAction::PayFine,
                ..
            } => {
                skip = Ledger {
                    cash: &mut cash,
                    houses: &mut houses,
                    board,
                }
                .apply_debt(events, i + 1, env.player, rules.jail_fine, None);
            }
            Event::CardDrawn { effect, .. } => match effect {
                CardEffect::CollectFromBank(amount) => {
                    cash[env.player] += *amount as i64;
                    cash_flow[env.player].card_net += *amount as i64;
                }
                CardEffect::PayBank(amount) => {
                    skip = Ledger {
                        cash: &mut cash,
                        houses: &mut houses,
                        board,
                    }
                    .apply_debt(events, i + 1, env.player, *amount, None);
                    cash_flow[env.player].card_net -= *amount as i64;
                }
                CardEffect::CollectFromEachPlayer(amount) => {
                    let mut offset = 1;
                    for (other, &is_bankrupt) in bankrupt.iter().enumerate() {
                        if other == env.player || is_bankrupt {
                            continue;
                        }
                        offset += Ledger {
                            cash: &mut cash,
                            houses: &mut houses,
                            board,
                        }
                        .apply_debt(
                            events,
                            i + offset,
                            other,
                            *amount,
                            Some(env.player),
                        );
                    }
                    skip = offset - 1;
                    cash_flow[env.player].card_net += *amount as i64 * (n as i64 - 1);
                }
                CardEffect::PayEachPlayer(amount) => {
                    let mut offset = 1;
                    for (other, &is_bankrupt) in bankrupt.iter().enumerate() {
                        if bankrupt[env.player] {
                            break;
                        }
                        if other == env.player || is_bankrupt {
                            continue;
                        }
                        offset += Ledger {
                            cash: &mut cash,
                            houses: &mut houses,
                            board,
                        }
                        .apply_debt(
                            events,
                            i + offset,
                            env.player,
                            *amount,
                            Some(other),
                        );
                    }
                    skip = offset - 1;
                    cash_flow[env.player].card_net -= *amount as i64 * (n as i64 - 1);
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
                // Whatever `player` had left (after any raise-cash mortgages
                // `apply_debt` already credited) goes to the actual payee;
                // applied uniformly here regardless of which debt triggered
                // it, matching `Game::bankrupt_player`.
                let remaining = cash[env.player].max(0);
                cash[env.player] = 0;
                if let Some(p) = payee {
                    cash[*p] += remaining;
                }
                bankrupt[env.player] = true;
                bankruptcies.push(BankruptcyRecord {
                    player: env.player,
                    turn: env.turn,
                    payee: *payee,
                });
                for space in 0..BOARD_SIZE {
                    if owner[space] != Some(env.player) {
                        continue;
                    }
                    houses[space] = 0;
                    owner[space] = *payee;
                    if let Some(new_owner) = payee {
                        check_monopoly(
                            board,
                            &owner,
                            space,
                            *new_owner,
                            env.turn,
                            &mut monopolies_completed,
                        );
                    }
                }
            }
            _ => {}
        }
        i += 1 + skip;
    }
    net_worth_by_turn.push(net_worth_snapshot(board, n, &cash, &owner, &houses));

    let property_roi = (0..BOARD_SIZE)
        .filter_map(|space| {
            let price = board.space(space).price()?;
            let house_cost = board.space(space).house_cost().unwrap_or(0);
            let increments = if houses[space] == 5 {
                5
            } else {
                houses[space] as u32
            };
            Some(PropertyRoi {
                space,
                owner: owner[space],
                rent_collected: rent_collected[space],
                cost_basis: price + house_cost * increments,
            })
        })
        .collect();

    PerGameStats {
        winner: result.winner,
        turns: result.turns,
        net_worth_by_turn,
        cash_flow,
        property_timeline,
        monopolies_completed,
        bankruptcies,
        dice_roll_counts,
        landing_counts,
        property_roi,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PlayerConfig;
    use crate::game::Game;

    fn players(strategies: &[&str]) -> Vec<PlayerConfig> {
        strategies
            .iter()
            .enumerate()
            .map(|(i, s)| PlayerConfig {
                name: format!("P{i}"),
                strategy: s.to_string(),
            })
            .collect()
    }

    #[test]
    fn dice_and_landing_counts_match_the_event_log() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let players = players(&["buy_all", "buy_none"]);
        let mut game = Game::new(rules.clone(), &players, 42).unwrap();
        let result = game.run_to_completion();
        let stats = compute_stats(&board, &players, &rules, &result);

        let roll_events = result
            .events
            .iter()
            .filter(|e| matches!(e.event, Event::RollDice { .. }))
            .count() as u64;
        let move_events = result
            .events
            .iter()
            .filter(|e| matches!(e.event, Event::Move { .. }))
            .count() as u64;
        assert_eq!(stats.dice_roll_counts.iter().sum::<u64>(), roll_events);
        assert_eq!(stats.landing_counts.iter().sum::<u64>(), move_events);
    }

    #[test]
    fn property_timeline_matches_successful_acquisitions() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let players = players(&["buy_all", "buy_none"]);
        let mut game = Game::new(rules.clone(), &players, 42).unwrap();
        let result = game.run_to_completion();
        let stats = compute_stats(&board, &players, &rules, &result);

        let acquisitions = result
            .events
            .iter()
            .filter(|e| matches!(e.event, Event::PurchaseDecision { bought: true, .. }))
            .count()
            + result
                .events
                .iter()
                .filter(|e| matches!(e.event, Event::AuctionWon { .. }))
                .count();
        assert_eq!(stats.property_timeline.len(), acquisitions);
        // Buy None never buys, so every acquired property ends up owned by
        // player 0, and its cost basis (list price plus any building
        // investment) is always at least the property's list price.
        for roi in &stats.property_roi {
            if let Some(owner) = roi.owner {
                assert_eq!(owner, 0);
                assert!(roi.cost_basis >= board.space(roi.space).price().unwrap());
            }
        }
    }

    #[test]
    fn net_worth_snapshot_matches_starting_cash_before_anything_happens() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let cash = vec![rules.starting_cash as i64; 2];
        let owner = vec![None; BOARD_SIZE];
        let houses = vec![0u8; BOARD_SIZE];
        assert_eq!(
            net_worth_snapshot(&board, 2, &cash, &owner, &houses),
            vec![rules.starting_cash, rules.starting_cash]
        );
    }

    #[test]
    fn apply_debt_leaves_cash_untouched_when_bankruptcy_follows() {
        // The caller's own `Event::Bankrupted` handling does the actual
        // zeroing-and-transfer (uniformly, regardless of which debt
        // triggered it) — `apply_debt` just needs to not also subtract the
        // amount that was never actually paid.
        let mut cash = vec![30i64, 500];
        let mut houses = vec![0u8; BOARD_SIZE];
        let board = Board::standard();
        let events = [EventEnvelope {
            turn: 1,
            player: 0,
            seq: 1,
            event: Event::Bankrupted { payee: Some(1) },
        }];
        let consumed = Ledger {
            cash: &mut cash,
            houses: &mut houses,
            board: &board,
        }
        .apply_debt(&events, 0, 0, 100, Some(1));
        assert_eq!(consumed, 0); // no raise-cash events to skip; the caller visits Bankrupted itself
        assert_eq!(cash, vec![30, 500]);
    }

    #[test]
    fn apply_debt_pays_in_full_when_no_bankruptcy_follows() {
        let mut cash = vec![200i64, 500];
        let mut houses = vec![0u8; BOARD_SIZE];
        let board = Board::standard();
        let consumed = Ledger {
            cash: &mut cash,
            houses: &mut houses,
            board: &board,
        }
        .apply_debt(&[], 0, 0, 100, Some(1));
        assert_eq!(consumed, 0);
        assert_eq!(cash, vec![100, 600]);
    }
}
