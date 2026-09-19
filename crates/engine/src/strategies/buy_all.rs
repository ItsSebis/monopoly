use crate::state::GameView;
use crate::strategy::{JailAction, PurchaseOffer, Strategy};

/// Cash reserve kept back after a purchase — small, since Buy All is fully
/// committed to accumulating property (see docs/player-strategies.md).
const RESERVE: i64 = 50;

#[derive(Debug, Default)]
pub struct BuyAll;

impl Strategy for BuyAll {
    fn decide_purchase(&mut self, view: &GameView, offer: &PurchaseOffer) -> bool {
        let player = view.state.current_player;
        view.player(player).cash - offer.price as i64 >= RESERVE
    }

    fn decide_jail_action(&mut self, view: &GameView) -> JailAction {
        let player = view.state.current_player;
        if view.player(player).cash >= view.rules.jail_fine as i64 {
            JailAction::PayFine
        } else {
            JailAction::RollForDoubles
        }
    }
}
