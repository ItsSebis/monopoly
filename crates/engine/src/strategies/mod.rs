mod buy_all;
mod buy_bad;
mod buy_good;
mod buy_none;

pub use buy_all::BuyAll;
pub use buy_bad::BuyBad;
pub use buy_good::BuyGood;
pub use buy_none::BuyNone;

use crate::strategy::Strategy;

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
