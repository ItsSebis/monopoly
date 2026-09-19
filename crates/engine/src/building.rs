//! Pure house/hotel building rules, kept separate from `Game` so they're
//! testable without spinning up a full game. Bank supply limits (a fixed,
//! shared count across the whole game) are checked in `game.rs` instead,
//! since they aren't a property of a single color group.

/// Whether building one house/hotel increment on `target_index` is allowed,
/// given the current house count (0 = none, 1-4 = houses, 5 = hotel) of
/// every property in its color group. The even-build rule (see
/// docs/game-rules.md) requires building up evenly: a property may not end
/// up more than one increment ahead of the group's least-built property.
pub fn can_build(group_houses: &[u8], target_index: usize, even_build_rule: bool) -> bool {
    let current = group_houses[target_index];
    if current >= 5 {
        return false; // already a hotel
    }
    if !even_build_rule {
        return true;
    }
    current == group_houses.iter().copied().min().unwrap_or(0)
}

/// Whether selling one house/hotel increment off `target_index` is allowed —
/// the even-build rule applies symmetrically when selling.
pub fn can_sell(group_houses: &[u8], target_index: usize, even_build_rule: bool) -> bool {
    let current = group_houses[target_index];
    if current == 0 {
        return false; // nothing built to sell
    }
    if !even_build_rule {
        return true;
    }
    current == group_houses.iter().copied().max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_build_rule_only_allows_building_the_least_built_property() {
        let group = [0, 0, 1]; // third property already has a house
        assert!(can_build(&group, 0, true));
        assert!(can_build(&group, 1, true));
        assert!(
            !can_build(&group, 2, true),
            "would put it 2 ahead of the others"
        );
    }

    #[test]
    fn even_build_rule_only_allows_selling_the_most_built_property() {
        let group = [0, 0, 1];
        assert!(!can_sell(&group, 0, true), "nothing built here to sell");
        assert!(can_sell(&group, 2, true));
    }

    #[test]
    fn disabling_even_build_rule_allows_any_uneven_build_or_sell() {
        let group = [0, 0, 3];
        assert!(can_build(&group, 2, false));
        assert!(can_sell(&group, 2, false));
    }

    #[test]
    fn a_hotel_cannot_be_built_on_further() {
        let group = [5, 4, 4];
        assert!(!can_build(&group, 0, true));
        assert!(!can_build(&group, 0, false));
    }
}
