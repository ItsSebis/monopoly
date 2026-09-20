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

/// A voluntary building decision, made once per turn (see
/// `Strategy::decide_build`). Invalid actions (even-build violations, no
/// bank supply left, wrong player, etc.) are silently skipped by the engine
/// rather than erroring — a strategy is a heuristic, not a guarantee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildAction {
    Build(usize),
    SellHouse(usize),
}

/// A cash-raising action under a payment shortfall (see
/// `Strategy::decide_mortgage`). Selling houses and mortgaging are the same
/// decision in practice — real bankruptcy resolution requires selling houses
/// on a group before mortgaging any property in it — so one hook and one
/// action type covers both, rather than splitting them across two hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MortgageAction {
    Mortgage(usize),
    SellHouse(usize),
}

/// A proposed trade from the acting player (`decide_trade`'s caller) to
/// `to`: `offered_*` leaves the proposer, `requested_*` is what they want
/// back from `to`. Only unmortgaged, house-free properties can ever be
/// traded (see `docs/game-rules.md#trading`) — the engine validates this,
/// not the strategy, so a strategy proposing an invalid trade is simply
/// refused rather than crashing anything, matching every other action type
/// here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeOffer {
    pub to: usize,
    pub offered_properties: Vec<usize>,
    pub offered_cash: u32,
    pub requested_properties: Vec<usize>,
    pub requested_cash: u32,
}

/// Every decision a player must make. The trait grows additively as each
/// mechanic is implemented (Phase 1 had only purchase/jail decisions);
/// Phase 2 adds building, raising cash under a shortfall, and auction
/// bidding.
///
/// `player` is passed explicitly rather than left for implementations to
/// read off `view.state.current_player`: that shortcut only happens to work
/// while the acting player and the current player are the same, which
/// `decide_auction_bid` breaks (it asks every player, not just the current
/// one, to bid).
pub trait Strategy: std::fmt::Debug {
    fn decide_purchase(&mut self, view: &GameView, player: usize, offer: &PurchaseOffer) -> bool;
    fn decide_jail_action(&mut self, view: &GameView, player: usize) -> JailAction;
    /// Called once at the end of `player`'s own turn. Only properties
    /// `player` owns are ever built on or sold by the returned actions.
    fn decide_build(&mut self, view: &GameView, player: usize) -> Vec<BuildAction>;
    /// Called when `player` owes `shortfall` more than their cash on hand.
    /// The engine applies returned actions in order until the shortfall is
    /// covered (or the actions run out), then re-checks affordability.
    fn decide_mortgage(
        &mut self,
        view: &GameView,
        player: usize,
        shortfall: u32,
    ) -> Vec<MortgageAction>;
    /// A sealed bid for `space`, currently up for auction: `None` (or `Some(0)`)
    /// to abstain. `player` doesn't see other players' bids — see
    /// docs/roadmap.md's Phase 2 auction design note for why auctions are
    /// modeled as a single sealed round rather than live ascending bidding.
    fn decide_auction_bid(&mut self, view: &GameView, player: usize, space: usize) -> Option<u32>;
    /// Called once at the end of `player`'s own turn, only when
    /// `RuleSet.trading_enabled` — at most one proposed trade per turn,
    /// mirroring `decide_build`'s once-per-turn shape. `None` to propose
    /// nothing this turn.
    fn decide_trade(&mut self, view: &GameView, player: usize) -> Option<TradeOffer>;
    /// Asks `player` (the trade's `to`) whether to accept `offer`. This is a
    /// deliberate addition beyond `docs/simulation-engine.md`'s original
    /// one-hook sketch: a trade means nothing without the other side's
    /// consent, the same reasoning that already makes `decide_auction_bid`
    /// ask every player rather than just the current one (see above) — see
    /// `docs/game-rules.md#trading` for the full rationale.
    fn decide_trade_response(&mut self, view: &GameView, player: usize, offer: &TradeOffer)
        -> bool;
}
