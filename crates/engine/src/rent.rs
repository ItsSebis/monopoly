//! Pure rent calculations, kept separate from `Game` so they're testable
//! without spinning up a full game (no RNG or state needed).

const RAILROAD_RENT_BY_COUNT: [u32; 4] = [25, 50, 100, 200];

/// Rent for a street with no buildings: double the base rent if the owner
/// holds the full color group, even with no houses built (see
/// docs/game-rules.md).
pub fn unimproved_street_rent(base_rent: u32, owner_holds_full_group: bool) -> u32 {
    if owner_holds_full_group {
        base_rent * 2
    } else {
        base_rent
    }
}

/// Rent for a street, given however many houses it has (`0` = unimproved,
/// `1..=4` = that many houses, `5` = a hotel). `house_rent` is the space's
/// `[1 house, 2, 3, 4, hotel]` table.
pub fn street_rent(
    base_rent: u32,
    owner_holds_full_group: bool,
    houses: u8,
    house_rent: [u32; 5],
) -> u32 {
    match houses {
        0 => unimproved_street_rent(base_rent, owner_holds_full_group),
        1..=4 => house_rent[houses as usize - 1],
        5 => house_rent[4],
        _ => unreachable!("a street has at most 4 houses or 1 hotel"),
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
    fn unimproved_street_rent_doubles_with_a_full_group() {
        assert_eq!(unimproved_street_rent(10, false), 10);
        assert_eq!(unimproved_street_rent(10, true), 20);
    }

    #[test]
    fn street_rent_uses_the_house_table_once_built() {
        let table = [10, 30, 90, 160, 250]; // Mediterranean Avenue's table
        assert_eq!(
            street_rent(2, true, 0, table),
            4,
            "unimproved: monopoly-doubled base rent"
        );
        assert_eq!(street_rent(2, true, 1, table), 10);
        assert_eq!(street_rent(2, true, 4, table), 160);
        assert_eq!(street_rent(2, true, 5, table), 250, "5 = hotel");
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
