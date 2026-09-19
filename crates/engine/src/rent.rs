//! Pure rent calculations, kept separate from `Game` so they're testable
//! without spinning up a full game (no RNG or state needed).

const RAILROAD_RENT_BY_COUNT: [u32; 4] = [25, 50, 100, 200];

/// Rent for a street: double the base rent if the owner holds the full
/// color group, even with no houses built (see docs/game-rules.md).
pub fn street_rent(base_rent: u32, owner_holds_full_group: bool) -> u32 {
    if owner_holds_full_group {
        base_rent * 2
    } else {
        base_rent
    }
}

/// Rent for a railroad, scaled by how many of the 4 the owner holds.
/// `owned_count` must be in 1..=4.
pub fn railroad_rent(owned_count: usize) -> u32 {
    RAILROAD_RENT_BY_COUNT[owned_count - 1]
}

/// Rent for a utility: 4x the dice roll with one held, 10x with both.
pub fn utility_rent(owned_count: usize, dice_total: u8) -> u32 {
    let multiplier = if owned_count >= 2 { 10 } else { 4 };
    multiplier * dice_total as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn street_rent_doubles_with_a_full_group_even_without_houses() {
        assert_eq!(street_rent(10, false), 10);
        assert_eq!(street_rent(10, true), 20);
    }

    #[test]
    fn railroad_rent_scales_with_count_owned() {
        assert_eq!(railroad_rent(1), 25);
        assert_eq!(railroad_rent(2), 50);
        assert_eq!(railroad_rent(3), 100);
        assert_eq!(railroad_rent(4), 200);
    }

    #[test]
    fn utility_rent_is_a_dice_roll_multiplier() {
        assert_eq!(utility_rent(1, 7), 28);
        assert_eq!(utility_rent(2, 7), 70);
    }
}
