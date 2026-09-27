mod buy_all;
mod buy_bad;
mod buy_good;
mod buy_none;
mod buy_optimal;
mod buy_shrewd;
mod configurable;

pub use buy_all::BuyAll;
pub use buy_bad::BuyBad;
pub use buy_good::BuyGood;
pub use buy_none::BuyNone;
pub use buy_shrewd::BuyShrewd;
pub use configurable::{
    AuctionPolicy, BuildPolicy, Configurable, ConfigurableParams, JailPolicy, TradePolicy,
    Valuation,
};

use crate::board::{ColorGroup, SpaceKind, BOARD_SIZE};
use crate::building::can_build;
use crate::state::GameView;
use crate::strategy::{BuildAction, JailAction, MortgageAction, Strategy, TradeOffer};

/// Every registered strategy id, in the same order `make_strategy` matches
/// them — the one source of truth for anything that needs to list them (e.g.
/// the browser's strategy dropdown in Phase 5), instead of a hand-maintained
/// duplicate.
pub const STRATEGY_IDS: &[&str] = &[
    "buy_all",
    "buy_good",
    "buy_bad",
    "buy_none",
    "buy_shrewd",
    "buy_optimal",
];

/// Construct a built-in strategy by its registered id (used by config files
/// and the CLI). `None` for an unrecognized id, so callers can report a
/// clear config error instead of panicking. Every built-in strategy here is
/// a plain data struct with no interior mutability, so each is `Send`
/// automatically.
pub fn make_strategy(id: &str) -> Option<Box<dyn Strategy + Send>> {
    match id {
        "buy_all" => Some(Box::new(BuyAll)),
        "buy_good" => Some(Box::new(BuyGood)),
        "buy_bad" => Some(Box::new(BuyBad)),
        "buy_none" => Some(Box::new(BuyNone)),
        "buy_shrewd" => Some(Box::new(BuyShrewd)),
        "buy_optimal" => Some(Box::new(Configurable(buy_optimal::PARAMS))),
        _ => {
            if let Some(json) = id.strip_prefix("cfg:") {
                let params: ConfigurableParams = serde_json::from_str(json).ok()?;
                // A negative reserve would make `cash_above_reserve`'s
                // `cash - reserve` subtraction (and every caller downstream
                // of it) meaningless, so reject it here rather than let a
                // malformed but syntactically-valid `cfg:` id (this is
                // reachable from untrusted input — e.g. a server replaying a
                // stored `PlayerConfig`) silently do something nonsensical.
                if params.reserve < 0 {
                    return None;
                }
                Some(Box::new(Configurable(params)))
            } else {
                None
            }
        }
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

/// Jail logic shared by Buy All and any `Configurable` using
/// `JailPolicy::PayIfAffordable`: pay the fine immediately whenever cash
/// allows, regardless of monopoly status — the simplest possible policy,
/// with no phase-dependence at all (contrast `patient_jail_action` and
/// `hotel_risk_jail_action`, both of which condition on monopoly state).
fn affordable_jail_action(view: &GameView, player: usize) -> JailAction {
    if view.player(player).cash >= view.rules.jail_fine as i64 {
        JailAction::PayFine
    } else {
        JailAction::RollForDoubles
    }
}

/// Phase-dependent jail policy keyed on *opponents'* hotel risk, not the
/// acting player's own monopolies (unlike `patient_jail_action`): leave fast
/// while no opponent has a built-up monopoly yet (the board is still open,
/// nothing dangerous to land on), stay once one does (avoiding the risk of
/// landing on it, at the cost of a slower re-roll) — see
/// docs/player-strategies.md's Jail section. Shared by Buy Shrewd and any
/// `Configurable` using `JailPolicy::HotelRisk`.
fn hotel_risk_jail_action(view: &GameView, player: usize) -> JailAction {
    let opponent_has_a_built_monopoly = (0..view.state.players.len())
        .filter(|&p| p != player && !view.state.players[p].bankrupt)
        .any(|p| {
            ColorGroup::ALL.into_iter().any(|group| {
                view.owns_full_group(p, group)
                    && view.group_house_counts(group).iter().any(|&h| h > 0)
            })
        });
    if !opponent_has_a_built_monopoly && view.player(player).cash >= view.rules.jail_fine as i64 {
        JailAction::PayFine
    } else {
        JailAction::RollForDoubles
    }
}

/// Minimum rent-to-price score for a street to be worth buying — and the
/// flat score railroads/utilities are given instead (see
/// `rent_to_price_score`). Shared by Buy Good and Buy Shrewd.
const RATIO_THRESHOLD: f64 = 0.06;
/// Score bonus applied when a purchase would complete a color group.
const MONOPOLY_BONUS: f64 = 0.15;

/// A rent-to-price heuristic, with a bonus for completing a monopoly.
/// Shared by purchase decisions and auction bids — see
/// docs/player-strategies.md. Buy Shrewd multiplies a landing-frequency
/// weight on top of this rather than scoring differently (see
/// `buy_shrewd::weighted_score`).
fn rent_to_price_score(view: &GameView, player: usize, space: usize) -> Option<f64> {
    match view.board.space(space) {
        SpaceKind::Street {
            group,
            base_rent,
            price,
            ..
        } => {
            let completes_monopoly = view
                .board
                .group_members(group)
                .all(|s| s == space || view.owner_of(s) == Some(player));
            let bonus = if completes_monopoly {
                MONOPOLY_BONUS
            } else {
                0.0
            };
            Some(base_rent as f64 / price as f64 + bonus)
        }
        // Railroads/utilities have no fixed base rent (it scales with how
        // many the buyer ends up holding), so they're valued as a steady,
        // moderate investment rather than scored on the street formula.
        SpaceKind::Railroad { .. } | SpaceKind::Utility { .. } => Some(RATIO_THRESHOLD),
        _ => None,
    }
}

/// A static per-group landing-frequency multiplier, from the Markov-chain
/// research cited in docs/player-strategies.md: Jail itself is the
/// single most-landed-on space, and the Orange/Red groups just past it
/// inherit that traffic (Orange the most, Red close behind) — a gap
/// docs/player-strategies.md's Buy Good section records as considered and
/// skipped there. Every other group is unweighted; this is a multiplier on
/// top of the shared rent-to-price-plus-monopoly-bonus score, not a
/// replacement for it.
fn landing_weight(group: ColorGroup) -> f64 {
    match group {
        ColorGroup::Orange => 1.3,
        ColorGroup::Red => 1.15,
        _ => 1.0,
    }
}

/// The shared `rent_to_price_score`, multiplied by `landing_weight` — and
/// still judged against the same `RATIO_THRESHOLD` as the plain score, since
/// the weighting, not the bar, is what differs. Railroads and utilities
/// belong to no color group, so they keep the shared score unchanged. Shared
/// by Buy Shrewd and any `Configurable` using `Valuation::Weighted`.
fn weighted_score(view: &GameView, player: usize, space: usize) -> Option<f64> {
    let base = rent_to_price_score(view, player, space)?;
    match view.board.space(space) {
        SpaceKind::Street { group, .. } => Some(base * landing_weight(group)),
        _ => Some(base),
    }
}

/// How much `player` can commit beyond `reserve` — the ceiling every bidding
/// strategy caps its auction bid at. `None` when already at or below the
/// reserve, which is also how a strategy abstains from the auction.
/// `saturating_sub` rather than plain subtraction: `reserve` ultimately
/// comes from `ConfigurableParams` (reachable from untrusted input via a
/// `cfg:` strategy id — see `make_strategy`), so this stays panic-free even
/// if a validation gap elsewhere ever lets an extreme value through.
fn cash_above_reserve(view: &GameView, player: usize, reserve: i64) -> Option<u32> {
    let spare = view.player(player).cash.saturating_sub(reserve);
    (spare > 0).then_some(spare as u32)
}

/// Whether a single other player already owns every other member of
/// `space`'s color group — the trigger for a denial bid (see
/// `cash_above_reserve`'s use at each call site): bidding up to full
/// affordability purely to block that player's monopoly, independent of
/// `space`'s own valuation. `false` for a non-street space (railroads/
/// utilities have no single "group" to complete this way). Shared by Buy
/// Shrewd and any `Configurable` using `AuctionPolicy::ValuationCappedWithDenial`.
fn denial_bid_applies(view: &GameView, player: usize, space: usize) -> bool {
    let SpaceKind::Street { group, .. } = view.board.space(space) else {
        return false;
    };
    let other_members: Vec<usize> = view
        .board
        .group_members(group)
        .filter(|&m| m != space)
        .collect();
    let Some(sole_owner) = other_members.first().and_then(|&m| view.owner_of(m)) else {
        return false;
    };
    sole_owner != player
        && other_members
            .iter()
            .all(|&m| view.owner_of(m) == Some(sole_owner))
}

/// A bid at `price * (1 + score)`, capped by what `player` can spare above
/// `reserve` — `None` (abstain) if `score` (the caller's own valuation of
/// `space`, already computed by whichever `Valuation` function it uses) is
/// below `RATIO_THRESHOLD`, `space` has no price, or there's no cash above
/// `reserve` to bid. Shared by Buy Good, Buy Shrewd's non-denial fallback,
/// and any `Configurable`'s `AuctionPolicy`.
fn valuation_capped_bid(
    score: Option<f64>,
    view: &GameView,
    player: usize,
    space: usize,
    reserve: i64,
) -> Option<u32> {
    let score = score.filter(|&s| s >= RATIO_THRESHOLD)?;
    let price = view.board.space(space).price()?;
    let valuation = (price as f64 * (1.0 + score)) as u32;
    Some(valuation.min(cash_above_reserve(view, player, reserve)?))
}

/// Building logic shared by Buy All, Buy Good, and Buy Shrewd (see
/// docs/player-strategies.md): build one increment at a time on every fully
/// owned group, cheapest-eligible-property first, stopping once cash would
/// drop below `reserve`. Buy All/Buy Good differ only in their reserve; Buy
/// Shrewd additionally sets `stop_before_hotel` for house-supply denial
/// (never converting 4 houses to a hotel, which would otherwise free 4
/// houses back to the bank's supply) — this one loop (rather than
/// near-duplicate code in each file) covers all three.
///
/// The plan is simulated locally against `view`'s snapshot — it doesn't
/// track the bank's house/hotel supply, since the engine already validates
/// and silently skips any action that supply can't cover (see
/// `BuildAction`'s doc comment), so a strategy overshooting supply is
/// harmless, not incorrect.
fn build_within_reserve(
    view: &GameView,
    player: usize,
    reserve: i64,
    stop_before_hotel: bool,
) -> Vec<BuildAction> {
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
                if stop_before_hotel && houses_by_group[group_index][member_index] == 4 {
                    continue;
                }
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
/// docs/player-strategies.md's Trading note) — the one trading heuristic
/// both use, at most one proposal per turn like `decide_build`.
fn propose_monopoly_completing_trade(view: &GameView, player: usize) -> Option<TradeOffer> {
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

        if let Some(spare) = find_reciprocal_spare(view, player, counterparty, group) {
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
/// participating in, that would complete a color group for `counterparty` —
/// a "spare" worth giving up in a direct swap rather than for cash.
/// `target` (the group `player` is trying to complete via this same trade)
/// is explicitly excluded: `player`'s own member of `target` would otherwise
/// satisfy every condition here too (it does complete `target` for
/// `counterparty`, and `player` owns no *other* member of it, precisely
/// because it's the one property being traded away) — offering it up would
/// swap `player`'s piece of the group for `counterparty`'s, mirroring the
/// exact same trade back with nobody ever completing anything.
fn find_reciprocal_spare(
    view: &GameView,
    player: usize,
    counterparty: usize,
    target: ColorGroup,
) -> Option<usize> {
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
        if group == target {
            return false;
        }
        let would_complete_for_counterparty = view
            .board
            .group_members(group)
            .all(|m| m == space || view.owner_of(m) == Some(counterparty));
        let player_owns_others_in_group = view
            .board
            .group_members(group)
            .any(|m| m != space && view.owner_of(m) == Some(player));
        would_complete_for_counterparty && !player_owns_others_in_group
    })
}

/// Whether `player` should accept an incoming `TradeOffer` (they'd receive
/// `offered_properties`/`offered_cash`, giving up `requested_properties`/
/// `requested_cash`) — shared by every strategy that can own property, since
/// all of them value a trade the same way: accept if it completes a
/// monopoly *after* the trade, or if it's a pure cash buyout that exceeds
/// the requested properties' combined list price.
fn accept_trade(view: &GameView, player: usize, offer: &TradeOffer) -> bool {
    // Judged on ownership *after* the swap: an incoming property only counts
    // if every other member of its group is still (or becomes) `player`'s —
    // not given away by this same trade's `requested_properties`, which a
    // check against current ownership alone would miss (accepting the exact
    // mirror of a trade that also takes away another piece of the group).
    let completes_a_monopoly = offer.offered_properties.iter().any(|&space| {
        let SpaceKind::Street { group, .. } = view.board.space(space) else {
            return false;
        };
        view.board.group_members(group).all(|m| {
            m == space
                || offer.offered_properties.contains(&m)
                || (view.owner_of(m) == Some(player) && !offer.requested_properties.contains(&m))
        })
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
    use crate::board::Board;
    use crate::rules::RuleSet;
    use crate::state::GameState;

    #[test]
    fn every_strategy_id_round_trips_through_make_strategy() {
        for &id in STRATEGY_IDS {
            assert!(
                make_strategy(id).is_some(),
                "STRATEGY_IDS lists {id}, but make_strategy doesn't recognize it"
            );
        }
    }

    fn two_player_state(rules: &RuleSet) -> GameState {
        GameState::new(rules, &["P0".to_string(), "P1".to_string()])
    }

    /// Regression for a bug where `find_reciprocal_spare` could offer up
    /// `player`'s own member of the group being traded for (it trivially
    /// "completes" the counterparty's group and isn't part of any *other*
    /// group `player` owns) — proposing the exact mirror of the trade being
    /// made, which two trading strategies would then swap back and forth
    /// forever without ever completing anything.
    #[test]
    fn find_reciprocal_spare_never_offers_a_member_of_the_target_group_itself() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let mut state = two_player_state(&rules);
        // Brown: player 0 owns Mediterranean (1), player 1 owns Baltic (3) —
        // the only "spare" `find_reciprocal_spare` could otherwise find is
        // player 0's own Mediterranean, since it has no other Brown property
        // and giving it up "completes" Brown for player 1.
        state.properties[1].owner = Some(0);
        state.properties[3].owner = Some(1);
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
            log_since_turn_start: &[],
        };

        let spare = find_reciprocal_spare(&view, 0, 1, ColorGroup::Brown);

        assert_eq!(
            spare, None,
            "the only candidate is player 0's own Mediterranean Avenue, in the excluded target group"
        );
    }

    /// End-to-end regression: proposing a trade for a 2-property group with
    /// no other groups in play must fall back to a cash offer (or nothing),
    /// never a direct swap of the two matching pieces.
    #[test]
    fn propose_monopoly_completing_trade_never_proposes_a_same_group_swap() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let mut state = two_player_state(&rules);
        state.properties[1].owner = Some(0);
        state.properties[3].owner = Some(1);
        state.players[0].cash = 10_000;
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
            log_since_turn_start: &[],
        };

        if let Some(offer) = propose_monopoly_completing_trade(&view, 0) {
            assert!(
                !offer.offered_properties.contains(&1),
                "must not offer away the exact property needed to complete the group: {offer:?}"
            );
        }
    }

    /// Regression for a bug where `accept_trade` judged "completes a
    /// monopoly" against *current* ownership only, so it accepted the exact
    /// mirror image of the same broken trade (receiving one piece of a group
    /// while simultaneously giving away the group's other piece).
    #[test]
    fn accept_trade_rejects_a_same_group_mirror_swap() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let mut state = two_player_state(&rules);
        state.properties[1].owner = Some(0); // Mediterranean — about to be offered to player 0
        state.properties[3].owner = Some(0); // Baltic — player 0 already owns this
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
            log_since_turn_start: &[],
        };
        let offer = TradeOffer {
            to: 0,
            offered_properties: vec![1],
            offered_cash: 0,
            requested_properties: vec![3],
            requested_cash: 0,
        };

        assert!(
            !accept_trade(&view, 0, &offer),
            "receiving Mediterranean while giving away Baltic completes nothing"
        );
    }
}
