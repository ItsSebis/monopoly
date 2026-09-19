use super::patient_jail_action;
use crate::state::GameView;
use crate::strategy::{BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy};

/// Never buys; used as a floor-line control. Jail decisions still use Buy
/// Good's logic, since jail behavior isn't tied to ownership.
#[derive(Debug, Default)]
pub struct BuyNone;

impl Strategy for BuyNone {
    fn decide_purchase(
        &mut self,
        _view: &GameView,
        _player: usize,
        _offer: &PurchaseOffer,
    ) -> bool {
        false
    }

    fn decide_jail_action(&mut self, view: &GameView, player: usize) -> JailAction {
        patient_jail_action(view, player)
    }

    fn decide_build(&mut self, _view: &GameView, _player: usize) -> Vec<BuildAction> {
        Vec::new() // owns nothing to build on
    }

    fn decide_mortgage(
        &mut self,
        _view: &GameView,
        _player: usize,
        _shortfall: u32,
    ) -> Vec<MortgageAction> {
        Vec::new() // owns nothing to mortgage
    }

    fn decide_auction_bid(
        &mut self,
        _view: &GameView,
        _player: usize,
        _space: usize,
    ) -> Option<u32> {
        None
    }
}
