use super::{
    accept_trade, build_within_reserve, cash_above_reserve, patient_jail_action,
    propose_monopoly_completing_trade, raise_cash_cheapest_first, rent_to_price_score,
    RATIO_THRESHOLD,
};
use crate::state::GameView;
use crate::strategy::{
    BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy, TradeOffer,
};

/// Cash reserve required after a purchase or build.
const RESERVE: i64 = 150;

/// Buys based on a rent-to-price heuristic and monopoly proximity rather than
/// raw affordability — see docs/player-strategies.md.
#[derive(Debug, Default)]
pub struct BuyGood;

impl Strategy for BuyGood {
    fn decide_purchase(&mut self, view: &GameView, player: usize, offer: &PurchaseOffer) -> bool {
        if view.player(player).cash - (offer.price as i64) < RESERVE {
            return false;
        }
        rent_to_price_score(view, player, offer.space).is_some_and(|s| s >= RATIO_THRESHOLD)
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
        let score = rent_to_price_score(view, player, space).filter(|&s| s >= RATIO_THRESHOLD)?;
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
