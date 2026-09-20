mod buy_all;
mod buy_bad;
mod buy_good;
mod buy_none;

pub use buy_all::BuyAll;
pub use buy_bad::BuyBad;
pub use buy_good::BuyGood;
pub use buy_none::BuyNone;

use crate::board::{ColorGroup, SpaceKind, BOARD_SIZE};
use crate::building::can_build;
use crate::state::GameView;
use crate::strategy::{BuildAction, JailAction, MortgageAction, Strategy, TradeOffer};

/// Every registered strategy id, in the same order `make_strategy` matches
/// them — the one source of truth for anything that needs to list them (e.g.
/// the browser's strategy dropdown in Phase 5), instead of a hand-maintained
/// duplicate.
pub const STRATEGY_IDS: &[&str] = &["buy_all", "buy_good", "buy_bad", "buy_none"];

/// Construct a built-in strategy by its registered id (used by config files
/// and the CLI). `None` for an unrecognized id, so callers can report a
/// clear config error instead of panicking.
pub fn make_strategy(id: &str) -> Option<Box<dyn Strategy>> {
    match id {
        "buy_all" => Some(Box::new(BuyAll)),
        "buy_good" => Some(Box::new(BuyGood)),
        "buy_bad" => Some(Box::new(BuyBad)),
        "buy_none" => Some(Box::new(BuyNone)),
        _ => None,
    }
}

/// Jail logic shared by Buy Good and Buy None (see docs/player-strategies.md):
/// stay (free re-roll attempt) while holding no monopoly, pay to guarantee an
/// immediate exit once a monopoly is actively earning rent.
fn patient_jail_action(view: &GameView, player: usize) -> JailAction {
    let holds_a_monopoly = ColorGroup::ALL
        .into_iter()
        .any(|group| view.owns_full_group(player, group));

    if holds_a_monopoly && view.player(player).cash >= view.rules.jail_fine as i64 {
        JailAction::PayFine
    } else {
        JailAction::RollForDoubles
    }
}

/// How much `player` can commit beyond `reserve` — the ceiling every bidding
/// strategy caps its auction bid at. `None` when already at or below the
/// reserve, which is also how a strategy abstains from the auction.
fn cash_above_reserve(view: &GameView, player: usize, reserve: i64) -> Option<u32> {
    let spare = view.player(player).cash - reserve;
    (spare > 0).then_some(spare as u32)
}

/// Building logic shared by Buy All and Buy Good (see
/// docs/player-strategies.md): build one increment at a time on every fully
/// owned group, cheapest-eligible-property first, stopping once cash would
/// drop below `reserve`. Both strategies differ only in their reserve, so
/// this one loop (rather than near-duplicate code in each file) covers both.
///
/// The plan is simulated locally against `view`'s snapshot — it doesn't
/// track the bank's house/hotel supply, since the engine already validates
/// and silently skips any action that supply can't cover (see
/// `BuildAction`'s doc comment), so a strategy overshooting supply is
/// harmless, not incorrect.
fn build_within_reserve(view: &GameView, player: usize, reserve: i64) -> Vec<BuildAction> {
    let mut cash = view.player(player).cash;
    let mut houses_by_group: Vec<Vec<u8>> = ColorGroup::ALL
        .iter()
        .map(|&g| view.group_house_counts(g))
        .collect();
    let mut actions = Vec::new();

    loop {
        let mut built_this_pass = false;
        for (group_index, group) in ColorGroup::ALL.into_iter().enumerate() {
            if !view.owns_full_group(player, group) {
                continue;
            }
            for (member_index, space) in view.board.group_members(group).enumerate() {
                let Some(cost) = view.board.space(space).house_cost() else {
                    continue;
                };
                let affordable = cash - cost as i64 >= reserve;
                if affordable
                    && can_build(
                        &houses_by_group[group_index],
                        member_index,
                        view.rules.even_build_rule,
                    )
                {
                    actions.push(BuildAction::Build(space));
                    houses_by_group[group_index][member_index] += 1;
                    cash -= cost as i64;
                    built_this_pass = true;
                }
            }
        }
        if !built_this_pass {
            return actions;
        }
    }
}

/// Cash-raising logic shared by every strategy that can own property (Buy
/// None never does, so it never needs this): sell houses and mortgage
/// unmortgaged properties, cheapest purchase price first, until `shortfall`
/// is covered. Every building in a color group is sold before any property
/// in it is mortgaged, matching the official rule.
///
/// Unlike `build_within_reserve`, where overshooting is harmless, this plan
/// has to be *legal*: the engine silently drops a sale the even-build rule
/// forbids or a mortgage on a still-built-up group, and every dropped action
/// is cash the strategy counted toward the shortfall but never received —
/// the difference between surviving a payment and going bankrupt holding a
/// full board. Hence the simulated house counts, and always selling from the
/// group's most-built property.
fn raise_cash_cheapest_first(
    view: &GameView,
    player: usize,
    shortfall: u32,
) -> Vec<MortgageAction> {
    let mut houses: Vec<u8> = (0..BOARD_SIZE).map(|s| view.property(s).houses).collect();
    let mut candidates: Vec<usize> = (0..BOARD_SIZE)
        .filter(|&s| view.owner_of(s) == Some(player) && !view.property(s).mortgaged)
        .collect();
    candidates.sort_by_key(|&s| view.board.space(s).price().unwrap_or(0));

    let mut actions = Vec::new();
    let mut raised: u32 = 0;
    for space in candidates {
        if raised >= shortfall {
            break;
        }
        // Railroads and utilities have no group to clear and go straight to
        // the mortgage below.
        let group_members: Vec<usize> = match view.board.space(space) {
            SpaceKind::Street { group, .. } => view.board.group_members(group).collect(),
            _ => Vec::new(),
        };
        while raised < shortfall {
            let Some(&target) = group_members.iter().max_by_key(|&&m| houses[m]) else {
                break;
            };
            if houses[target] == 0 {
                break;
            }
            actions.push(MortgageAction::SellHouse(target));
            houses[target] -= 1;
            raised += view.board.space(target).house_cost().unwrap_or(0) / 2;
        }
        if raised < shortfall && group_members.iter().all(|&m| houses[m] == 0) {
            if let Some(price) = view.board.space(space).price() {
                actions.push(MortgageAction::Mortgage(space));
                raised += price / 2;
            }
        }
    }
    actions
}

/// Looks across every color group `player` doesn't yet fully own for one
/// where every remaining member is held by a single other player, and
/// proposes trading for it — a direct swap if `player` has a spare property
/// that would complete a *different* group of that same counterparty's,
/// otherwise a cash offer at a 1.5x premium over the missing property/ies'
/// combined list price. Shared by Buy All and Buy Good (see
/// docs/player-strategies.md's Trading note) - the one trading heuristic
/// both use, at most one proposal per turn like `decide_build`.
pub fn propose_monopoly_completing_trade(view: &GameView, player: usize) -> Option<TradeOffer> {
    for group in ColorGroup::ALL {
        let members: Vec<usize> = view.board.group_members(group).collect();
        if view.owns_full_group(player, group)
            || !members.iter().any(|&m| view.owner_of(m) == Some(player))
        {
            continue;
        }
        let missing: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&m| view.owner_of(m) != Some(player))
            .collect();
        let Some(counterparty) = view.owner_of(missing[0]) else {
            continue; // an unowned member: nobody to trade with for it yet
        };
        let tradeable = missing.iter().all(|&m| {
            view.owner_of(m) == Some(counterparty)
                && !view.property(m).mortgaged
                && view.property(m).houses == 0
        });
        if !tradeable {
            continue;
        }

        if let Some(spare) = find_reciprocal_spare(view, player, counterparty) {
            return Some(TradeOffer {
                to: counterparty,
                offered_properties: vec![spare],
                offered_cash: 0,
                requested_properties: missing,
                requested_cash: 0,
            });
        }

        let value: u32 = missing
            .iter()
            .filter_map(|&m| view.board.space(m).price())
            .sum();
        let offer_cash = value + value / 2;
        if view.player(player).cash >= offer_cash as i64 {
            return Some(TradeOffer {
                to: counterparty,
                offered_properties: Vec::new(),
                offered_cash: offer_cash,
                requested_properties: missing,
                requested_cash: 0,
            });
        }
    }
    None
}

/// A property `player` owns, outside any group they're otherwise
/// participating in, that would complete a color group for `counterparty` -
/// a "spare" worth giving up in a direct swap rather than for cash.
fn find_reciprocal_spare(view: &GameView, player: usize, counterparty: usize) -> Option<usize> {
    (0..BOARD_SIZE).find(|&space| {
        if view.owner_of(space) != Some(player)
            || view.property(space).mortgaged
            || view.property(space).houses > 0
        {
            return false;
        }
        let SpaceKind::Street { group, .. } = view.board.space(space) else {
            return false;
        };
        let mut members = view.board.group_members(group);
        let would_complete_for_counterparty =
            members.all(|m| m == space || view.owner_of(m) == Some(counterparty));
        let player_owns_others_in_group = view
            .board
            .group_members(group)
            .any(|m| m != space && view.owner_of(m) == Some(player));
        would_complete_for_counterparty && !player_owns_others_in_group
    })
}

/// Whether `player` should accept an incoming `TradeOffer` (they'd receive
/// `offered_properties`/`offered_cash`, giving up `requested_properties`/
/// `requested_cash`) - shared by every strategy that can own property, since
/// all of them value a trade the same way: accept if it completes a
/// monopoly, or if it's a pure cash buyout that exceeds the requested
/// properties' combined list price.
pub fn accept_trade(view: &GameView, player: usize, offer: &TradeOffer) -> bool {
    let completes_a_monopoly = offer.offered_properties.iter().any(|&space| {
        let SpaceKind::Street { group, .. } = view.board.space(space) else {
            return false;
        };
        view.board
            .group_members(group)
            .all(|m| m == space || view.owner_of(m) == Some(player))
    });
    if completes_a_monopoly {
        return true;
    }
    if offer.offered_properties.is_empty() && !offer.requested_properties.is_empty() {
        let value: u32 = offer
            .requested_properties
            .iter()
            .filter_map(|&s| view.board.space(s).price())
            .sum();
        return offer.offered_cash > value;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_strategy_id_round_trips_through_make_strategy() {
        for &id in STRATEGY_IDS {
            assert!(
                make_strategy(id).is_some(),
                "STRATEGY_IDS lists {id}, but make_strategy doesn't recognize it"
            );
        }
    }
}
