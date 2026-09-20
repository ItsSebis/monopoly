use super::{
    accept_trade, build_within_reserve, cash_above_reserve, patient_jail_action,
    propose_monopoly_completing_trade, raise_cash_cheapest_first,
};
use crate::board::SpaceKind;
use crate::state::GameView;
use crate::strategy::{
    BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy, TradeOffer,
};

/// Cash reserve required after a purchase or build.
const RESERVE: i64 = 150;
/// Minimum rent-to-price score to want a street.
const RATIO_THRESHOLD: f64 = 0.06;
/// Score bonus applied when a purchase would complete a color group.
const MONOPOLY_BONUS: f64 = 0.15;

/// A rent-to-price heuristic, with a bonus for completing a monopoly.
/// Shared by purchase decisions and auction bids — see
/// docs/player-strategies.md.
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
            Some(base_rent as f64 / price as f64 + bonus)
        }
        // Railroads/utilities have no fixed base rent (it scales with how
        // many the buyer ends up holding), so they're valued as a steady,
        // moderate investment rather than scored on the street formula.
        SpaceKind::Railroad { .. } | SpaceKind::Utility { .. } => Some(RATIO_THRESHOLD),
        _ => None,
    }
}

/// Buys based on a rent-to-price heuristic and monopoly proximity rather than
/// raw affordability — see docs/player-strategies.md.
#[derive(Debug, Default)]
pub struct BuyGood;

impl Strategy for BuyGood {
    fn decide_purchase(&mut self, view: &GameView, player: usize, offer: &PurchaseOffer) -> bool {
        if view.player(player).cash - (offer.price as i64) < RESERVE {
            return false;
        }
        score(view, player, offer.space).is_some_and(|s| s >= RATIO_THRESHOLD)
    }

    fn decide_jail_action(&mut self, view: &GameView, player: usize) -> JailAction {
        patient_jail_action(view, player)
    }

    fn decide_build(&mut self, view: &GameView, player: usize) -> Vec<BuildAction> {
        build_within_reserve(view, player, RESERVE, false)
    }

    fn decide_mortgage(
        &mut self,
        view: &GameView,
        player: usize,
        shortfall: u32,
    ) -> Vec<MortgageAction> {
        raise_cash_cheapest_first(view, player, shortfall)
    }

    /// Bids its own valuation of the space — the same score that drives its
    /// purchases — capped by what it can spare above its reserve, and only on
    /// spaces it would have bought outright.
    fn decide_auction_bid(&mut self, view: &GameView, player: usize, space: usize) -> Option<u32> {
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
