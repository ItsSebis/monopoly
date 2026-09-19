use serde::Serialize;

use crate::state::GameView;

pub struct PurchaseOffer {
    pub space: usize,
    pub price: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JailAction {
    PayFine,
    RollForDoubles,
}

/// Every decision a player must make. Phase 1 only has purchase and jail
/// decisions to make (no houses, mortgages, or auctions exist yet) — the
/// trait grows additively in later phases alongside those mechanics, rather
/// than declaring hooks nothing calls yet.
///
/// `player` is passed explicitly rather than left for implementations to
/// read off `view.state.current_player`: in Phase 1 the two are always the
/// same, but Phase 2's auction bidding (`decide_auction_bid`) will ask
/// *non-current* players to decide, at which point that shortcut would
/// silently evaluate the wrong player.
pub trait Strategy: std::fmt::Debug {
    fn decide_purchase(&mut self, view: &GameView, player: usize, offer: &PurchaseOffer) -> bool;
    fn decide_jail_action(&mut self, view: &GameView, player: usize) -> JailAction;
}
