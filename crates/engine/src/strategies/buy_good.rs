use crate::board::SpaceKind;
use crate::state::GameView;
use crate::strategy::{patient_jail_action, JailAction, PurchaseOffer, Strategy};

/// Cash reserve required after a purchase.
const RESERVE: i64 = 150;
/// Minimum rent-to-price score to buy a street.
const RATIO_THRESHOLD: f64 = 0.06;
/// Score bonus applied when a purchase would complete a color group.
const MONOPOLY_BONUS: f64 = 0.15;

/// Buys based on a rent-to-price heuristic and monopoly proximity rather than
/// raw affordability — see docs/player-strategies.md.
#[derive(Debug, Default)]
pub struct BuyGood;

impl Strategy for BuyGood {
    fn decide_purchase(&mut self, view: &GameView, offer: &PurchaseOffer) -> bool {
        let player = view.state.current_player;
        if view.player(player).cash - (offer.price as i64) < RESERVE {
            return false;
        }
        let score = match view.board.space(offer.space) {
            SpaceKind::Street { group, base_rent, price } => {
                let ratio = base_rent as f64 / price as f64;
                let completes_monopoly = view
                    .board
                    .group_members(group)
                    .all(|s| s == offer.space || view.owner_of(s) == Some(player));
                ratio + if completes_monopoly { MONOPOLY_BONUS } else { 0.0 }
            }
            // Railroads/utilities have no fixed base rent (it scales with how
            // many the buyer ends up holding), so they're valued as a steady,
            // moderate investment rather than scored on the street formula.
            SpaceKind::Railroad { .. } | SpaceKind::Utility { .. } => RATIO_THRESHOLD,
            _ => return false,
        };
        score >= RATIO_THRESHOLD
    }

    fn decide_jail_action(&mut self, view: &GameView) -> JailAction {
        patient_jail_action(view, view.state.current_player)
    }
}
