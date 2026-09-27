use super::{
    AuctionPolicy, BuildPolicy, Configurable, ConfigurableParams, JailPolicy, TradePolicy,
    Valuation,
};
use crate::state::GameView;
use crate::strategy::{
    BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy, TradeOffer,
};

/// The configuration `monopoly sweep` (Phase 8, docs/roadmap.md) found —
/// see docs/strategy-search-results.md for the full search and numbers this
/// summarizes. Every `Configurable` axis (`strategies/configurable.rs`) was
/// searched as 24 stage-1 combos (`Valuation::Weighted`, `reserve: 100`)
/// against the 5 named strategies, across 4 ruleset environments
/// (`examples/sweep_*.toml`), then refined over reserve/valuation. This
/// combo had the best mean rank across all 4 rulesets' stage-1 rankings
/// (2.25, next best 3.25 — see docs/strategy-search-results.md), and
/// `reserve: 50` then beat every other refined reserve/valuation variant by
/// mean rank (1.25) too.
const PARAMS: ConfigurableParams = ConfigurableParams {
    // A landing-frequency-weighted rent-to-price threshold (same scoring as
    // Buy Shrewd) beat the plain ratio in every one of the 4 rulesets'
    // stage-1 rankings.
    valuation: Valuation::Weighted,
    // The simplest jail policy (Buy All's) won out over Buy Good's
    // own-monopoly-based patience and Buy Shrewd's opponent-hotel-risk
    // policy — neither more sophisticated policy justified its cost here.
    jail: JailPolicy::PayIfAffordable,
    // Building all the way to a hotel (not stopping at 4 houses to deny the
    // bank's house supply) outranked house-supply denial in every ruleset.
    build: BuildPolicy {
        stop_before_hotel: false,
    },
    // The plain valuation-capped bid beat Buy Shrewd's denial-bidding
    // addition — denial bidding's extra cash commitment didn't pay for
    // itself against this panel.
    auction: AuctionPolicy::ValuationCapped,
    // The monopoly-completing trade heuristic Buy All/Buy Good/Buy Shrewd
    // already share.
    trade: TradePolicy::MonopolyCompleting,
    // The thinnest refined reserve ($50, matching Buy All's) beat every
    // other value tried ($100/$150/$200/$300) by mean rank across all 4
    // rulesets.
    reserve: 50,
};

/// The strongest configuration Phase 8's empirical search
/// (`monopoly sweep`) found: `Configurable` with the axis choices `PARAMS`
/// hardcodes above. In practice this reads as "Buy All's low reserve,
/// simple jail policy, and full-to-hotel building, but gated on Buy
/// Shrewd's landing-frequency-weighted valuation instead of buying
/// everything affordable" — buying indiscriminately (Buy All) and being
/// maximally cautious/sophisticated (Buy Shrewd) both lost to this
/// in-between combination. See docs/strategy-search-results.md for the
/// full per-ruleset numbers and honest caveats (the margin over the
/// runner-up combo is real but not huge in every ruleset).
#[derive(Debug)]
pub struct BuyOptimal(Configurable);

impl Default for BuyOptimal {
    fn default() -> Self {
        BuyOptimal(Configurable(PARAMS))
    }
}

impl Strategy for BuyOptimal {
    fn decide_purchase(&mut self, view: &GameView, player: usize, offer: &PurchaseOffer) -> bool {
        self.0.decide_purchase(view, player, offer)
    }

    fn decide_jail_action(&mut self, view: &GameView, player: usize) -> JailAction {
        self.0.decide_jail_action(view, player)
    }

    fn decide_build(&mut self, view: &GameView, player: usize) -> Vec<BuildAction> {
        self.0.decide_build(view, player)
    }

    fn decide_mortgage(
        &mut self,
        view: &GameView,
        player: usize,
        shortfall: u32,
    ) -> Vec<MortgageAction> {
        self.0.decide_mortgage(view, player, shortfall)
    }

    fn decide_auction_bid(&mut self, view: &GameView, player: usize, space: usize) -> Option<u32> {
        self.0.decide_auction_bid(view, player, space)
    }

    fn decide_trade(&mut self, view: &GameView, player: usize) -> Option<TradeOffer> {
        self.0.decide_trade(view, player)
    }

    fn decide_trade_response(
        &mut self,
        view: &GameView,
        player: usize,
        offer: &TradeOffer,
    ) -> bool {
        self.0.decide_trade_response(view, player, offer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hardcodes_the_sweep_winning_params() {
        let BuyOptimal(Configurable(params)) = BuyOptimal::default();
        assert_eq!(params, PARAMS);
    }
}
