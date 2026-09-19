mod buy_all;
mod buy_bad;
mod buy_good;
mod buy_none;

pub use buy_all::BuyAll;
pub use buy_bad::BuyBad;
pub use buy_good::BuyGood;
pub use buy_none::BuyNone;

use crate::board::ColorGroup;
use crate::state::GameView;
use crate::strategy::{JailAction, Strategy};

/// Construct a built-in strategy by its registered id (used by config files
/// and the CLI). `None` for an unrecognized id, so callers can report a
/// clear config error instead of panicking.
pub fn make_strategy(id: &str) -> Option<Box<dyn Strategy>> {
    match id {
        "buy_all" => Some(Box::new(BuyAll)),
        "buy_good" => Some(Box::new(BuyGood)),
        "buy_bad" => Some(Box::new(BuyBad)),
        "buy_none" => Some(Box::new(BuyNone)),
        _ => None,
    }
}

/// Jail logic shared by Buy Good and Buy None (see docs/player-strategies.md):
/// stay (free re-roll attempt) while holding no monopoly, pay to guarantee an
/// immediate exit once a monopoly is actively earning rent.
fn patient_jail_action(view: &GameView, player: usize) -> JailAction {
    let holds_a_monopoly = ColorGroup::ALL
        .into_iter()
        .any(|group| view.owns_full_group(player, group));

    if holds_a_monopoly && view.player(player).cash >= view.rules.jail_fine as i64 {
        JailAction::PayFine
    } else {
        JailAction::RollForDoubles
    }
}
