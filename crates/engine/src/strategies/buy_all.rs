use super::{build_within_reserve, raise_cash_cheapest_first};
use crate::state::GameView;
use crate::strategy::{BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy};

/// Cash reserve kept back after a purchase or build — small, since Buy All
/// is fully committed to accumulating property (see docs/player-strategies.md).
const RESERVE: i64 = 50;

/// Buys every property it can still afford, builds on its monopolies as
/// soon as it can, and pays its way out of jail whenever it can — see
/// docs/player-strategies.md.
#[derive(Debug, Default)]
pub struct BuyAll;

impl Strategy for BuyAll {
    fn decide_purchase(&mut self, view: &GameView, player: usize, offer: &PurchaseOffer) -> bool {
        view.player(player).cash - offer.price as i64 >= RESERVE
    }

    fn decide_jail_action(&mut self, view: &GameView, player: usize) -> JailAction {
        if view.player(player).cash >= view.rules.jail_fine as i64 {
            JailAction::PayFine
        } else {
            JailAction::RollForDoubles
        }
    }

    fn decide_build(&mut self, view: &GameView, player: usize) -> Vec<BuildAction> {
        build_within_reserve(view, player, RESERVE)
    }

    fn decide_mortgage(
        &mut self,
        view: &GameView,
        player: usize,
        shortfall: u32,
    ) -> Vec<MortgageAction> {
        raise_cash_cheapest_first(view, player, shortfall)
    }

    fn decide_auction_bid(&mut self, view: &GameView, player: usize, _space: usize) -> Option<u32> {
        let max_bid = view.player(player).cash - RESERVE;
        (max_bid > 0).then_some(max_bid as u32)
    }
}
