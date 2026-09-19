use crate::state::GameView;
use crate::strategy::{JailAction, PurchaseOffer, Strategy};

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
}
