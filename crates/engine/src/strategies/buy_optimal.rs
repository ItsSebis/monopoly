use super::{AuctionPolicy, BuildPolicy, ConfigurableParams, JailPolicy, TradePolicy, Valuation};

/// The configuration Phase 8's `monopoly sweep` search found — registered as
/// the `"buy_optimal"` strategy id (`make_strategy`, `strategies/mod.rs`,
/// which builds it as `Configurable(PARAMS)` directly rather than through a
/// wrapper type). See docs/strategy-search-results.md for the full search
/// and numbers; the summary, checked across `--seed 1` through `5`:
///
/// - `valuation: Weighted`, `build.stop_before_hotel: false`,
///   `trade: MonopolyCompleting`, and `reserve: 50` were the clear winner on
///   every single seed (reserve strictly monotonic — win rate drops at every
///   step from $50 up to $300 — and `Weighted` beat `RentToPrice` every
///   time), so these are solid findings, not a coin flip that happened to
///   land once.
/// - `jail` and `auction` are **not** solid: which of `JailPolicy`'s 3 and
///   `AuctionPolicy`'s 2 values wins flips across seeds, with mean-rank
///   margins over the runner-up small enough to be within noise. Across the
///   5 seeds checked, `JailPolicy::HotelRisk` and
///   `AuctionPolicy::ValuationCappedWithDenial` — Buy Shrewd's own choice
///   for both — tied for the win most often (3 of 5), so that's the
///   tiebreak used below; treat it as "the most common tie," not "the
///   proven best."
pub(crate) const PARAMS: ConfigurableParams = ConfigurableParams {
    valuation: Valuation::Weighted,
    jail: JailPolicy::HotelRisk,
    build: BuildPolicy {
        stop_before_hotel: false,
    },
    auction: AuctionPolicy::ValuationCappedWithDenial,
    trade: TradePolicy::MonopolyCompleting,
    reserve: 50,
};
