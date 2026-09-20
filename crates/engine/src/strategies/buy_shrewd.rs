use super::{
    accept_trade, build_within_reserve, cash_above_reserve, propose_monopoly_completing_trade,
    raise_cash_cheapest_first,
};
use crate::board::{ColorGroup, SpaceKind};
use crate::state::GameView;
use crate::strategy::{
    BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy, TradeOffer,
};

/// Cash reserve kept back after a purchase or build — between Buy All's $50
/// and Buy Good's $150, matching this strategy's overall "more deliberate
/// than Buy All, more aggressive than Buy Good" character.
const RESERVE: i64 = 100;
/// Minimum weighted score to want a street (same threshold Buy Good uses;
/// the weighting itself, not the bar, is what differs — see `score`).
const RATIO_THRESHOLD: f64 = 0.06;
/// Score bonus applied when a purchase would complete a color group.
const MONOPOLY_BONUS: f64 = 0.15;

/// A static per-group landing-frequency multiplier, from the Markov-chain
/// research cited in docs/player-strategies.md: Jail itself is the
/// single most-landed-on space, and the Orange/Red groups just past it
/// inherit that traffic (Orange the most, Red close behind) — no built-in
/// strategy weights purchases or bids by this today (Buy Good's own doc
/// comment notes it was considered and skipped). Every other group is
/// unweighted; this is a multiplier on top of Buy Good's existing
/// rent-to-price-plus-monopoly-bonus score, not a replacement for it.
fn landing_weight(group: ColorGroup) -> f64 {
    match group {
        ColorGroup::Orange => 1.3,
        ColorGroup::Red => 1.15,
        _ => 1.0,
    }
}

/// Buy Good's rent-to-price-plus-monopoly-bonus score, multiplied by
/// `landing_weight` — see its doc comment.
fn score(view: &GameView, player: usize, space: usize) -> Option<f64> {
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
            Some((base_rent as f64 / price as f64 + bonus) * landing_weight(group))
        }
        SpaceKind::Railroad { .. } | SpaceKind::Utility { .. } => Some(RATIO_THRESHOLD),
        _ => None,
    }
}

/// Phase-dependent jail policy keyed on *opponents'* hotel risk, not the
/// acting player's own monopolies (unlike `patient_jail_action`, which Buy
/// Good/Buy None use): leave fast while no opponent has a built-up monopoly
/// yet (the board is still open, nothing dangerous to land on), stay once
/// one does (avoiding the risk of landing on it, at the cost of a slower
/// re-roll) — see docs/player-strategies.md's Jail section.
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

/// Combines every gap docs/player-strategies.md's "where the built-ins
/// diverge from this" section names into one strategy, rather than four
/// separate registrations: landing-frequency-weighted valuation (`score`),
/// house-supply-denial building (never converts to a hotel, keeping the
/// bank's fixed 32-house stock locked up), an opponent-hotel-risk jail
/// policy (`hotel_risk_jail_action`), and auction denial bidding (below).
/// Trading uses the same monopoly-completing heuristic Buy All/Buy Good
/// share.
#[derive(Debug, Default)]
pub struct BuyShrewd;

impl Strategy for BuyShrewd {
    fn decide_purchase(&mut self, view: &GameView, player: usize, offer: &PurchaseOffer) -> bool {
        if view.player(player).cash - (offer.price as i64) < RESERVE {
            return false;
        }
        score(view, player, offer.space).is_some_and(|s| s >= RATIO_THRESHOLD)
    }

    fn decide_jail_action(&mut self, view: &GameView, player: usize) -> JailAction {
        hotel_risk_jail_action(view, player)
    }

    /// `stop_before_hotel: true` — denial building: develop every held
    /// monopoly to 4 houses but never convert to a hotel, since hoteling
    /// frees 4 houses back to the bank's supply rather than keeping it
    /// locked up.
    fn decide_build(&mut self, view: &GameView, player: usize) -> Vec<BuildAction> {
        build_within_reserve(view, player, RESERVE, true)
    }

    fn decide_mortgage(
        &mut self,
        view: &GameView,
        player: usize,
        shortfall: u32,
    ) -> Vec<MortgageAction> {
        raise_cash_cheapest_first(view, player, shortfall)
    }

    /// Denial bidding: if a single other player already owns every other
    /// member of `space`'s group, bid up to full affordability regardless of
    /// this strategy's own valuation, purely to block that player's
    /// monopoly. Otherwise bids its own (landing-frequency-weighted)
    /// valuation, the same way Buy Good does.
    fn decide_auction_bid(&mut self, view: &GameView, player: usize, space: usize) -> Option<u32> {
        if let SpaceKind::Street { group, .. } = view.board.space(space) {
            let other_members: Vec<usize> = view
                .board
                .group_members(group)
                .filter(|&m| m != space)
                .collect();
            if let Some(sole_owner) = other_members.first().and_then(|&m| view.owner_of(m)) {
                let one_player_holds_the_rest = sole_owner != player
                    && other_members
                        .iter()
                        .all(|&m| view.owner_of(m) == Some(sole_owner));
                if one_player_holds_the_rest {
                    return cash_above_reserve(view, player, RESERVE);
                }
            }
        }
        let score = score(view, player, space).filter(|&s| s >= RATIO_THRESHOLD)?;
        let price = view.board.space(space).price()?;
        let valuation = (price as f64 * (1.0 + score)) as u32;
        Some(valuation.min(cash_above_reserve(view, player, RESERVE)?))
    }

    fn decide_trade(&mut self, view: &GameView, player: usize) -> Option<TradeOffer> {
        propose_monopoly_completing_trade(view, player)
    }

    fn decide_trade_response(
        &mut self,
        view: &GameView,
        player: usize,
        offer: &TradeOffer,
    ) -> bool {
        accept_trade(view, player, offer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::rules::RuleSet;
    use crate::state::GameState;

    fn two_player_state(rules: &RuleSet) -> GameState {
        GameState::new(rules, &["P0".to_string(), "P1".to_string()])
    }

    #[test]
    fn score_applies_the_landing_frequency_weight() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let state = two_player_state(&rules);
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
        };
        // St. James Place: Orange, space 16.
        let SpaceKind::Street {
            base_rent, price, ..
        } = board.space(16)
        else {
            panic!("expected a street");
        };
        let unweighted_ratio = base_rent as f64 / price as f64;
        let weighted = score(&view, 0, 16).unwrap();
        assert!(
            (weighted - unweighted_ratio * 1.3).abs() < 1e-9,
            "expected the Orange 1.3x multiplier applied on top of the plain rent-to-price ratio"
        );
    }

    #[test]
    fn decide_build_never_converts_four_houses_to_a_hotel() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let mut state = two_player_state(&rules);
        state.properties[1].owner = Some(0); // Mediterranean Avenue
        state.properties[3].owner = Some(0); // Baltic Avenue
        state.properties[1].houses = 4;
        state.properties[3].houses = 4;
        state.players[0].cash = 10_000;
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
        };

        let actions = BuyShrewd.decide_build(&view, 0);

        assert!(
            actions.is_empty(),
            "should never propose converting 4 houses to a hotel: {actions:?}"
        );
    }

    #[test]
    fn jail_action_flips_once_an_opponent_holds_a_built_monopoly() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let mut state = two_player_state(&rules);
        state.players[0].cash = 1000;
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
        };
        assert_eq!(
            BuyShrewd.decide_jail_action(&view, 0),
            JailAction::PayFine,
            "no opponent monopoly yet - leave quickly"
        );

        state.properties[1].owner = Some(1);
        state.properties[3].owner = Some(1);
        state.properties[1].houses = 1; // player 1's Brown monopoly is built up
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
        };
        assert_eq!(
            BuyShrewd.decide_jail_action(&view, 0),
            JailAction::RollForDoubles,
            "an opponent's built monopoly is now a landing risk - stay"
        );
    }

    #[test]
    fn auction_bid_is_a_denial_bid_when_one_opponent_holds_the_rest_of_the_group() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let mut state = two_player_state(&rules);
        state.properties[3].owner = Some(1); // player 1 already owns Baltic Avenue
        state.players[0].cash = 500;
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
        };

        // Mediterranean Avenue (space 1) is up for auction: winning it would
        // complete player 1's Brown monopoly, so this is bid up to full
        // affordability regardless of the space's own (low) valuation.
        let bid = BuyShrewd.decide_auction_bid(&view, 0, 1);

        assert_eq!(bid, Some(500 - RESERVE as u32));
    }
}
