//! A composable strategy (Phase 8, docs/roadmap.md): every axis the five
//! named strategies vary along, expressed as data (`ConfigurableParams`)
//! instead of a new Rust type per combination. This is what
//! `monopoly sweep` (`crates/cli/src/sweep.rs`) actually searches over, via
//! the `cfg:{json}` strategy id (`make_strategy`, `strategies/mod.rs`) — a
//! combination doesn't need its own registered name to be run through a
//! batch.
//!
//! Not every axis a named strategy varies on is represented here (Buy Bad's
//! inverse-overbid auction logic and Buy None's total abstention have no
//! analog below) — only the axes the sweep actually searches. `Configurable`
//! also does *not* generally reproduce Buy All: Buy All buys/bids on
//! anything affordable with no valuation threshold at all, while every
//! `Configurable` purchase/bid is gated by `RATIO_THRESHOLD` regardless of
//! parameters — this module's tests check a narrow fixture where the two
//! happen to agree (see
//! `configurable_matches_buy_all_on_a_narrow_overlap_fixture`'s doc
//! comment), not general equivalence. Buy Good and Buy Shrewd, by contrast,
//! *are* reproduced exactly, since their own decision logic is already
//! exactly what `Configurable` expresses.

use super::{
    accept_trade, affordable_jail_action, build_within_reserve, cash_above_reserve,
    denial_bid_applies, hotel_risk_jail_action, patient_jail_action,
    propose_monopoly_completing_trade, raise_cash_cheapest_first, rent_to_price_score,
    valuation_capped_bid, weighted_score, RATIO_THRESHOLD,
};
use crate::state::GameView;
use crate::strategy::{
    BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy, TradeOffer,
};

/// Which shared scoring function values a candidate property — the same
/// score drives both purchase decisions and auction bids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Valuation {
    /// The plain rent-to-price-plus-monopoly-bonus score (`rent_to_price_score`;
    /// Buy Good/Buy Bad's heuristic).
    RentToPrice,
    /// `rent_to_price_score`, multiplied by the Orange/Red landing-frequency
    /// weight (`weighted_score`; Buy Shrewd's heuristic).
    Weighted,
}

/// Which jail hook to use — see the three functions in `strategies/mod.rs`
/// this dispatches to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JailPolicy {
    /// `affordable_jail_action`: pay whenever cash allows, no phase-dependence.
    PayIfAffordable,
    /// `patient_jail_action`: stay while holding no monopoly, pay once one is
    /// actively earning rent.
    Patient,
    /// `hotel_risk_jail_action`: leave fast while no opponent holds a built
    /// monopoly, stay once one does.
    HotelRisk,
}

/// Building policy — currently just the one axis `build_within_reserve`
/// already exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BuildPolicy {
    /// Develop every held monopoly up to 4 houses but never convert to a
    /// hotel (Buy Shrewd's house-supply-denial building), instead of
    /// building all the way to a hotel when affordable (Buy All/Buy Good).
    pub stop_before_hotel: bool,
}

/// Auction bidding policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuctionPolicy {
    /// Bid its own valuation (per `Valuation`), capped by what it can spare
    /// above `reserve` — Buy Good's approach.
    ValuationCapped,
    /// The same, plus a denial bid (up to full affordability) whenever a
    /// single other player already owns every other member of the space's
    /// group — Buy Shrewd's approach.
    ValuationCappedWithDenial,
}

/// Trading policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradePolicy {
    /// Never propose or accept a trade (Buy Bad/Buy None's approach) —
    /// distinct from `RuleSet.trading_enabled` being off, since the engine
    /// only calls these hooks at all when trading is enabled (see
    /// `Game::maybe_trade`); this is what a `Configurable` does if asked
    /// while it *is* enabled.
    Never,
    /// `propose_monopoly_completing_trade`/`accept_trade` — Buy All/Buy
    /// Good/Buy Shrewd's shared heuristic.
    MonopolyCompleting,
}

/// Every axis `Configurable` varies on, plus the shared cash reserve. See
/// the module doc comment for what this can and can't express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConfigurableParams {
    pub valuation: Valuation,
    pub jail: JailPolicy,
    pub build: BuildPolicy,
    pub auction: AuctionPolicy,
    pub trade: TradePolicy,
    pub reserve: i64,
}

impl Default for ConfigurableParams {
    /// Mirrors Buy Shrewd's own parameters — the most sophisticated built-in,
    /// and a reasonable default for any caller (e.g. a test fixture) that
    /// wants "a" `Configurable` without picking every axis by hand.
    fn default() -> Self {
        ConfigurableParams {
            valuation: Valuation::Weighted,
            jail: JailPolicy::HotelRisk,
            build: BuildPolicy {
                stop_before_hotel: true,
            },
            auction: AuctionPolicy::ValuationCappedWithDenial,
            trade: TradePolicy::MonopolyCompleting,
            reserve: 100,
        }
    }
}

/// A strategy built entirely from `ConfigurableParams` — see the module doc
/// comment. Constructed directly (`Configurable(params)`) or via the
/// `cfg:{json}` strategy id (`make_strategy`, `strategies/mod.rs`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Configurable(pub ConfigurableParams);

impl Configurable {
    /// The shared score for `space`, per `self.0.valuation` — the one piece
    /// both `decide_purchase` and `decide_auction_bid` need.
    fn score(&self, view: &GameView, player: usize, space: usize) -> Option<f64> {
        match self.0.valuation {
            Valuation::RentToPrice => rent_to_price_score(view, player, space),
            Valuation::Weighted => weighted_score(view, player, space),
        }
    }
}

impl Strategy for Configurable {
    fn decide_purchase(&mut self, view: &GameView, player: usize, offer: &PurchaseOffer) -> bool {
        if view.player(player).cash - (offer.price as i64) < self.0.reserve {
            return false;
        }
        self.score(view, player, offer.space)
            .is_some_and(|s| s >= RATIO_THRESHOLD)
    }

    fn decide_jail_action(&mut self, view: &GameView, player: usize) -> JailAction {
        match self.0.jail {
            JailPolicy::PayIfAffordable => affordable_jail_action(view, player),
            JailPolicy::Patient => patient_jail_action(view, player),
            JailPolicy::HotelRisk => hotel_risk_jail_action(view, player),
        }
    }

    fn decide_build(&mut self, view: &GameView, player: usize) -> Vec<BuildAction> {
        build_within_reserve(view, player, self.0.reserve, self.0.build.stop_before_hotel)
    }

    fn decide_mortgage(
        &mut self,
        view: &GameView,
        player: usize,
        shortfall: u32,
    ) -> Vec<MortgageAction> {
        raise_cash_cheapest_first(view, player, shortfall)
    }

    /// Mirrors `BuyShrewd::decide_auction_bid`'s denial branch, gated on
    /// `AuctionPolicy::ValuationCappedWithDenial`; otherwise the plain
    /// valuation-capped bid every `Valuation`-scoring strategy shares.
    fn decide_auction_bid(&mut self, view: &GameView, player: usize, space: usize) -> Option<u32> {
        if self.0.auction == AuctionPolicy::ValuationCappedWithDenial
            && denial_bid_applies(view, player, space)
        {
            return cash_above_reserve(view, player, self.0.reserve);
        }
        valuation_capped_bid(
            self.score(view, player, space),
            view,
            player,
            space,
            self.0.reserve,
        )
    }

    fn decide_trade(&mut self, view: &GameView, player: usize) -> Option<TradeOffer> {
        match self.0.trade {
            TradePolicy::Never => None,
            TradePolicy::MonopolyCompleting => propose_monopoly_completing_trade(view, player),
        }
    }

    fn decide_trade_response(
        &mut self,
        view: &GameView,
        player: usize,
        offer: &TradeOffer,
    ) -> bool {
        match self.0.trade {
            TradePolicy::Never => false,
            TradePolicy::MonopolyCompleting => accept_trade(view, player, offer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::rules::RuleSet;
    use crate::state::GameState;
    use crate::strategies::{BuyAll, BuyGood, BuyShrewd};

    fn two_player_state(rules: &RuleSet) -> GameState {
        GameState::new(rules, &["P0".to_string(), "P1".to_string()])
    }

    const BUY_ALL_PARAMS: ConfigurableParams = ConfigurableParams {
        valuation: Valuation::RentToPrice,
        jail: JailPolicy::PayIfAffordable,
        build: BuildPolicy {
            stop_before_hotel: false,
        },
        auction: AuctionPolicy::ValuationCapped,
        trade: TradePolicy::MonopolyCompleting,
        reserve: 50,
    };

    const BUY_GOOD_PARAMS: ConfigurableParams = ConfigurableParams {
        valuation: Valuation::RentToPrice,
        jail: JailPolicy::Patient,
        build: BuildPolicy {
            stop_before_hotel: false,
        },
        auction: AuctionPolicy::ValuationCapped,
        trade: TradePolicy::MonopolyCompleting,
        reserve: 150,
    };

    // Buy Shrewd's params happen to equal `ConfigurableParams::default()`
    // (see its doc comment) — used directly below rather than duplicated
    // here as a second, easily-drifting copy of the same values.

    /// A handful of property/cash fixtures exercised against every purchase
    /// offer's price/space in play, not just one — cheap street, expensive
    /// street, railroad, and (for Buy All, which ignores valuation entirely)
    /// a case right at the reserve boundary.
    fn purchase_offers() -> Vec<PurchaseOffer> {
        vec![
            PurchaseOffer {
                space: 1,
                price: 60,
            }, // Mediterranean (Brown)
            PurchaseOffer {
                space: 16,
                price: 300,
            }, // St. James Place (Orange)
            PurchaseOffer {
                space: 39,
                price: 400,
            }, // Boardwalk (Dark Blue)
            PurchaseOffer {
                space: 5,
                price: 200,
            }, // Reading Railroad
        ]
    }

    /// `Configurable` does NOT generally reproduce Buy All (see the module
    /// doc comment): Buy All buys/bids on anything affordable with no
    /// valuation threshold, while every `Configurable` purchase/bid is
    /// gated by `RATIO_THRESHOLD` regardless of parameters. This fixture is
    /// deliberately constructed so every checked decision falls in the
    /// narrow region where the two happen to agree — build/jail logic is
    /// genuinely identical between them, and purchase/auction agree here
    /// only because (a) the shared state already owns both Brown properties
    /// (see below), which pushes Mediterranean's score over threshold via
    /// the monopoly bonus rather than its own (sub-threshold) rent-to-price
    /// ratio, and (b) the auction check's cash is squeezed so the reserve
    /// cap, not the valuation, binds. It is not evidence that `Configurable`
    /// can express Buy All's actual "buy/bid anything affordable" behavior
    /// in general — it provably can't (a `RATIO_THRESHOLD`-gated purchase on
    /// a genuinely low-value property, e.g. Mediterranean *without* the
    /// monopoly bonus, returns `false` where Buy All returns `true`).
    #[test]
    fn configurable_matches_buy_all_on_a_narrow_overlap_fixture() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let mut state = two_player_state(&rules);
        state.players[0].cash = 500;
        state.properties[1].owner = Some(0);
        state.properties[3].owner = Some(0);
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
        };

        for offer in purchase_offers() {
            assert_eq!(
                Configurable(BUY_ALL_PARAMS).decide_purchase(&view, 0, &offer),
                BuyAll.decide_purchase(&view, 0, &offer),
                "purchase mismatch for space {}",
                offer.space
            );
        }
        assert_eq!(
            Configurable(BUY_ALL_PARAMS).decide_build(&view, 0),
            BuyAll.decide_build(&view, 0)
        );
        assert_eq!(
            Configurable(BUY_ALL_PARAMS).decide_jail_action(&view, 0),
            BuyAll.decide_jail_action(&view, 0)
        );

        // Buy All's own `decide_auction_bid` ignores valuation entirely
        // (always bids everything above its reserve, on anything); a
        // `Configurable` with `AuctionPolicy::ValuationCapped` only matches
        // that when the reserve cap — not the valuation — is what actually
        // binds, so cash is constrained here to make sure it is. St. James
        // Place clears `RATIO_THRESHOLD` comfortably, so its valuation is
        // far above the $10 of headroom this leaves.
        let mut auction_state = state.clone();
        auction_state.players[0].cash = 60;
        let auction_view = GameView {
            board: &board,
            rules: &rules,
            state: &auction_state,
        };
        assert_eq!(
            Configurable(BUY_ALL_PARAMS).decide_auction_bid(&auction_view, 0, 16),
            BuyAll.decide_auction_bid(&auction_view, 0, 16)
        );
    }

    #[test]
    fn configurable_matches_buy_good_on_purchase_and_build_and_jail_and_auction() {
        let board = Board::standard();
        let rules = RuleSet::default();
        let mut state = two_player_state(&rules);
        state.players[0].cash = 800;
        state.properties[1].owner = Some(0);
        state.properties[3].owner = Some(0);
        state.properties[1].houses = 1;
        state.properties[3].houses = 1;
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
        };

        for offer in purchase_offers() {
            assert_eq!(
                Configurable(BUY_GOOD_PARAMS).decide_purchase(&view, 0, &offer),
                BuyGood.decide_purchase(&view, 0, &offer),
                "purchase mismatch for space {}",
                offer.space
            );
        }
        assert_eq!(
            Configurable(BUY_GOOD_PARAMS).decide_build(&view, 0),
            BuyGood.decide_build(&view, 0)
        );
        assert_eq!(
            Configurable(BUY_GOOD_PARAMS).decide_jail_action(&view, 0),
            BuyGood.decide_jail_action(&view, 0)
        );
        for space in [1usize, 16, 39, 5] {
            assert_eq!(
                Configurable(BUY_GOOD_PARAMS).decide_auction_bid(&view, 0, space),
                BuyGood.decide_auction_bid(&view, 0, space),
                "auction bid mismatch for space {space}"
            );
        }
    }

    #[test]
    fn configurable_matches_buy_shrewd_on_purchase_and_build_and_jail_and_auction() {
        let buy_shrewd_params = ConfigurableParams::default();
        let board = Board::standard();
        let rules = RuleSet::default();
        let mut state = two_player_state(&rules);
        state.players[0].cash = 800;
        // Player 1 owns the rest of Brown, so Mediterranean's auction is a
        // denial bid for both BuyShrewd and its Configurable equivalent.
        state.properties[3].owner = Some(1);
        let view = GameView {
            board: &board,
            rules: &rules,
            state: &state,
        };

        for offer in purchase_offers() {
            assert_eq!(
                Configurable(buy_shrewd_params).decide_purchase(&view, 0, &offer),
                BuyShrewd.decide_purchase(&view, 0, &offer),
                "purchase mismatch for space {}",
                offer.space
            );
        }

        let mut built_up_state = state.clone();
        built_up_state.properties[3].owner = None;
        built_up_state.properties[1].owner = Some(0);
        built_up_state.properties[3].owner = Some(0);
        built_up_state.properties[1].houses = 4;
        built_up_state.properties[3].houses = 4;
        built_up_state.players[0].cash = 10_000;
        let built_up_view = GameView {
            board: &board,
            rules: &rules,
            state: &built_up_state,
        };
        assert_eq!(
            Configurable(buy_shrewd_params).decide_build(&built_up_view, 0),
            BuyShrewd.decide_build(&built_up_view, 0)
        );

        assert_eq!(
            Configurable(buy_shrewd_params).decide_jail_action(&view, 0),
            BuyShrewd.decide_jail_action(&view, 0),
            "no opponent holds a built monopoly in this fixture"
        );

        // The no-opponent-monopoly case above is exactly where
        // `JailPolicy::HotelRisk` and `JailPolicy::PayIfAffordable` happen to
        // agree (both pay if affordable) — so it alone can't distinguish a
        // `Configurable` that actually dispatches to `hotel_risk_jail_action`
        // from one that doesn't. Check the case that does: an opponent
        // holding a built monopoly, where the two policies diverge.
        let mut opponent_built_state = two_player_state(&rules);
        opponent_built_state.players[0].cash = 800;
        opponent_built_state.properties[1].owner = Some(1);
        opponent_built_state.properties[3].owner = Some(1);
        opponent_built_state.properties[1].houses = 1; // player 1's Brown monopoly is built up
        let opponent_built_view = GameView {
            board: &board,
            rules: &rules,
            state: &opponent_built_state,
        };
        assert_eq!(
            Configurable(buy_shrewd_params).decide_jail_action(&opponent_built_view, 0),
            BuyShrewd.decide_jail_action(&opponent_built_view, 0),
            "an opponent's built monopoly should flip both to RollForDoubles"
        );

        for space in [1usize, 16, 39, 5] {
            assert_eq!(
                Configurable(buy_shrewd_params).decide_auction_bid(&view, 0, space),
                BuyShrewd.decide_auction_bid(&view, 0, space),
                "auction bid mismatch for space {space}"
            );
        }
    }

    #[test]
    fn make_strategy_recognizes_a_cfg_prefixed_id() {
        let json = serde_json::to_string(&ConfigurableParams::default()).unwrap();
        let id = format!("cfg:{json}");
        assert!(super::super::make_strategy(&id).is_some());
    }

    #[test]
    fn make_strategy_rejects_malformed_cfg_json() {
        assert!(super::super::make_strategy("cfg:not json").is_none());
    }
}
