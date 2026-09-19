use super::{cash_above_reserve, raise_cash_cheapest_first};
use crate::board::SpaceKind;
use crate::state::GameView;
use crate::strategy::{BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy};

/// The smallest reserve of the buying strategies — Buy Bad overspends
/// relative to its cash position (see docs/player-strategies.md).
const RESERVE: i64 = 20;

/// A deliberately suboptimal baseline strategy, used to give batch analysis a
/// clear "worse" reference point.
#[derive(Debug, Default)]
pub struct BuyBad;

impl Strategy for BuyBad {
    fn decide_purchase(&mut self, view: &GameView, player: usize, offer: &PurchaseOffer) -> bool {
        view.player(player).cash - offer.price as i64 >= RESERVE
    }

    fn decide_jail_action(&mut self, _view: &GameView, _player: usize) -> JailAction {
        // Rolls for doubles even when it could afford to leave sooner.
        JailAction::RollForDoubles
    }

    fn decide_build(&mut self, _view: &GameView, _player: usize) -> Vec<BuildAction> {
        // Its persistently thin cash position (from buying low-value
        // properties down to a $20 reserve) means it essentially never
        // accumulates enough surplus to build — implementing "never" is a
        // faithful approximation of "rarely" (see docs/player-strategies.md).
        Vec::new()
    }

    fn decide_mortgage(
        &mut self,
        view: &GameView,
        player: usize,
        shortfall: u32,
    ) -> Vec<MortgageAction> {
        raise_cash_cheapest_first(view, player, shortfall)
    }

    /// The inverse of Buy Good's heuristic: the worse the rent-to-price
    /// ratio, the more this strategy overbids on it.
    fn decide_auction_bid(&mut self, view: &GameView, player: usize, space: usize) -> Option<u32> {
        let price = view.board.space(space).price()?;
        let ratio = match view.board.space(space) {
            SpaceKind::Street { base_rent, .. } => base_rent as f64 / price as f64,
            _ => 0.05, // railroads/utilities: no fixed ratio, treated as mediocre
        };
        let overbid_factor = (0.20 - ratio).max(0.0) * 4.0;
        let bid = (price as f64 * (1.0 + overbid_factor)) as u32;
        Some(bid.min(cash_above_reserve(view, player, RESERVE)?))
    }
}
