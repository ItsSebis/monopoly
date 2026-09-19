use crate::state::GameView;
use crate::strategy::{patient_jail_action, JailAction, PurchaseOffer, Strategy};

/// Never buys; used as a floor-line control. Jail decisions still use Buy
/// Good's logic, since jail behavior isn't tied to ownership.
#[derive(Debug, Default)]
pub struct BuyNone;

impl Strategy for BuyNone {
    fn decide_purchase(&mut self, _view: &GameView, _offer: &PurchaseOffer) -> bool {
        false
    }

    fn decide_jail_action(&mut self, view: &GameView) -> JailAction {
        patient_jail_action(view, view.state.current_player)
    }
}
