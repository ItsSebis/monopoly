//! Pure tax calculations, kept separate from `Game` so they're testable
//! without spinning up a full game.

use crate::rules::IncomeTaxMode;

/// The income tax due, given the player's net worth (cash plus the
/// undiscounted purchase price of everything they own). `Choice` always
/// resolves to whichever amount is cheaper — see `IncomeTaxMode`'s doc
/// comment for why this is a fixed rule rather than a `Strategy` decision.
pub fn income_tax_due(mode: IncomeTaxMode, net_worth: u32) -> u32 {
    match mode {
        IncomeTaxMode::Flat { amount } => amount,
        IncomeTaxMode::Percentage { rate } => (net_worth as f64 * rate).round() as u32,
        IncomeTaxMode::Choice { flat_amount } => {
            let percentage = (net_worth as f64 * 0.10).round() as u32;
            flat_amount.min(percentage)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_mode_ignores_net_worth() {
        assert_eq!(
            income_tax_due(IncomeTaxMode::Flat { amount: 200 }, 5000),
            200
        );
    }

    #[test]
    fn percentage_mode_scales_with_net_worth() {
        assert_eq!(
            income_tax_due(IncomeTaxMode::Percentage { rate: 0.10 }, 3000),
            300
        );
    }

    #[test]
    fn choice_mode_picks_the_cheaper_option() {
        assert_eq!(
            income_tax_due(IncomeTaxMode::Choice { flat_amount: 200 }, 1000),
            100
        );
        assert_eq!(
            income_tax_due(IncomeTaxMode::Choice { flat_amount: 200 }, 5000),
            200
        );
    }
}
