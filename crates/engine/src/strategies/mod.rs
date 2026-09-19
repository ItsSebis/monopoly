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
use crate::strategy::{BuildAction, JailAction, MortgageAction, Strategy};

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
