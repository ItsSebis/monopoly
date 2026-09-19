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

/// The full `RuleSet` from `docs/data-model.md`, as of Phase 2: mortgaging
/// and card decks (Phase 2) added `even_build_rule`, `auction_on_decline`,
/// and `free_parking_pot`. `max_turns` stays deferred to Phase 3's batch
/// runner, which is what actually needs a configurable turn cap.
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
        }
    }
}
