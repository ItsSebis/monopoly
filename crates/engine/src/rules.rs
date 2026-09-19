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

/// The subset of the eventual `RuleSet` (see `docs/data-model.md`) that
/// Phase 1's mechanics actually use. Fields for house-building, auctions,
/// mortgaging, and free-parking pot are added in later phases alongside
/// those mechanics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    pub starting_cash: u32,
    pub go_salary: u32,
    pub jail_fine: u32,
    pub luxury_tax: u32,
    pub income_tax_mode: IncomeTaxMode,
}

impl Default for RuleSet {
    fn default() -> Self {
        RuleSet {
            starting_cash: 1500,
            go_salary: 200,
            jail_fine: 50,
            luxury_tax: 75,
            income_tax_mode: IncomeTaxMode::Choice { flat_amount: 200 },
        }
    }
}
