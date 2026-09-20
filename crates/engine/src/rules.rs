use serde::{Deserialize, Serialize};

/// How Income Tax is charged. `Choice` always resolves to whichever amount is
/// cheaper for the player — real Monopoly leaves this as a player choice, but
/// no rational player (built-in or custom) benefits from picking the more
/// expensive option, so Phase 1 resolves it as a fixed engine rule rather
/// than a `Strategy` decision hook.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum IncomeTaxMode {
    Flat { amount: u32 },
    Percentage { rate: f64 },
    Choice { flat_amount: u32 },
}

fn default_true() -> bool {
    true
}

/// The full `RuleSet` from `docs/data-model.md`, as of Phase 3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    pub starting_cash: u32,
    pub go_salary: u32,
    pub jail_fine: u32,
    pub luxury_tax: u32,
    pub income_tax_mode: IncomeTaxMode,
    /// Within a color group, no property may have more than one more
    /// house/hotel-increment than the group's least-built property.
    #[serde(default = "default_true")]
    pub even_build_rule: bool,
    /// Whether a declined purchase goes to auction (official rules) or
    /// simply stays unowned until someone else lands on it.
    #[serde(default = "default_true")]
    pub auction_on_decline: bool,
    /// Whether money paid to the bank accumulates in a pot that Free
    /// Parking collects (a common house rule; off matches official rules).
    #[serde(default)]
    pub free_parking_pot: bool,
    /// Optional cap on game length, mainly so a batch run (`docs/roadmap.md`
    /// Phase 3) can bound its own worst-case cost. Unset, a single game still
    /// falls back to the engine's internal safety valve.
    #[serde(default)]
    pub max_turns: Option<u32>,
    /// A common house rule: landing exactly on GO pays double the normal
    /// salary. Off matches official rules (a flat salary for landing on or
    /// passing GO alike).
    #[serde(default)]
    pub double_go_salary: bool,
    /// A common house rule: ignore the bank's fixed 32-house/12-hotel supply
    /// when building. Off matches official rules.
    #[serde(default)]
    pub unlimited_houses: bool,
    /// Player-initiated trading (`Strategy::decide_trade`/`decide_trade_response`,
    /// `docs/roadmap.md` Phase 7). Off by default, matching every other
    /// optional rule here — turning it on with the same strategy code
    /// otherwise unchanged is the intended before/after comparison.
    #[serde(default)]
    pub trading_enabled: bool,
}

impl Default for RuleSet {
    fn default() -> Self {
        RuleSet {
            starting_cash: 1500,
            go_salary: 200,
            jail_fine: 50,
            luxury_tax: 75,
            income_tax_mode: IncomeTaxMode::Choice { flat_amount: 200 },
            even_build_rule: true,
            auction_on_decline: true,
            free_parking_pot: false,
            max_turns: None,
            double_go_salary: false,
            unlimited_houses: false,
            trading_enabled: false,
        }
    }
}
