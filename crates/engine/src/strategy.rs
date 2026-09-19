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
pub trait Strategy: std::fmt::Debug {
    fn decide_purchase(&mut self, view: &GameView, offer: &PurchaseOffer) -> bool;
    fn decide_jail_action(&mut self, view: &GameView) -> JailAction;
}

/// Jail logic shared by Buy Good and Buy None (see docs/player-strategies.md):
/// stay (free re-roll attempt) while holding no monopoly, pay to guarantee an
/// immediate exit once a monopoly is actively earning rent.
pub fn patient_jail_action(view: &GameView, player: usize) -> JailAction {
    let holds_a_monopoly = [
        crate::board::ColorGroup::Brown,
        crate::board::ColorGroup::LightBlue,
        crate::board::ColorGroup::Pink,
        crate::board::ColorGroup::Orange,
        crate::board::ColorGroup::Red,
        crate::board::ColorGroup::Yellow,
        crate::board::ColorGroup::Green,
        crate::board::ColorGroup::DarkBlue,
    ]
    .into_iter()
    .any(|g| view.owns_full_group(player, g));

    if holds_a_monopoly && view.player(player).cash >= view.rules.jail_fine as i64 {
        JailAction::PayFine
    } else {
        JailAction::RollForDoubles
    }
}
