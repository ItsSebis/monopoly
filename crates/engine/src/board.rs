//! Static data for the standard 40-space Monopoly board.

pub const BOARD_SIZE: usize = 40;
pub const JAIL_SPACE: usize = 10;
pub const RAILROAD_SPACES: [usize; 4] = [5, 15, 25, 35];
pub const UTILITY_SPACES: [usize; 2] = [12, 28];

/// The street spaces of each color group, indexed by `ColorGroup as usize`
/// (so this table's row order must match the `ColorGroup` variant order).
/// Precomputed rather than derived by scanning the board, because
/// `owns_full_group` sits in the per-rent hot path and a 40-space scan there
/// dominates the lookup it performs. `group_members_match_the_board` keeps
/// this table honest against `Board::standard`.
const GROUP_MEMBERS: [&[usize]; 8] = [
    &[1, 3],
    &[6, 8, 9],
    &[11, 13, 14],
    &[16, 18, 19],
    &[21, 23, 24],
    &[26, 27, 29],
    &[31, 32, 34],
    &[37, 39],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorGroup {
    Brown,
    LightBlue,
    Pink,
    Orange,
    Red,
    Yellow,
    Green,
    DarkBlue,
}

impl ColorGroup {
    /// Every group, in `GROUP_MEMBERS` row order.
    pub const ALL: [ColorGroup; 8] = [
        ColorGroup::Brown,
        ColorGroup::LightBlue,
        ColorGroup::Pink,
        ColorGroup::Orange,
        ColorGroup::Red,
        ColorGroup::Yellow,
        ColorGroup::Green,
        ColorGroup::DarkBlue,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceKind {
    Go,
    Street {
        group: ColorGroup,
        price: u32,
        base_rent: u32,
        /// Rent with 1-4 houses (indices 0-3) and with a hotel (index 4).
        /// Official values — see docs/game-rules.md.
        house_rent: [u32; 5],
        /// Cost to build one house (or the 5th increment, a hotel) on this
        /// property. Fixed per color group in the real game.
        house_cost: u32,
    },
    Railroad {
        price: u32,
    },
    Utility {
        price: u32,
    },
    IncomeTax,
    LuxuryTax,
    Chance,
    CommunityChest,
    Jail,
    FreeParking,
    GoToJail,
}

impl SpaceKind {
    /// The bank purchase price, for ownable spaces.
    pub fn price(&self) -> Option<u32> {
        match self {
            SpaceKind::Street { price, .. }
            | SpaceKind::Railroad { price }
            | SpaceKind::Utility { price } => Some(*price),
            _ => None,
        }
    }

    /// The cost to build (or sell, at the same price) one house/hotel
    /// increment on this street.
    pub fn house_cost(&self) -> Option<u32> {
        match self {
            SpaceKind::Street { house_cost, .. } => Some(*house_cost),
            _ => None,
        }
    }
}

/// The fixed, standard-rules board layout. There is only one board in Phase 1
/// (no board variants), so this is a plain lookup table rather than
/// configuration.
pub struct Board {
    spaces: [SpaceKind; BOARD_SIZE],
}

impl Board {
    pub fn standard() -> Self {
        use ColorGroup::*;
        use SpaceKind::*;

        // Prices, base rents, house-rent tables, and house costs are the
        // standard official values for every property (verified internally
        // consistent: base_rent * 2 always equals the documented monopoly
        // rent). See docs/game-rules.md.
        Board {
            spaces: [
                Go,
                Street {
                    group: Brown,
                    price: 60,
                    base_rent: 2,
                    house_rent: [10, 30, 90, 160, 250],
                    house_cost: 50,
                },
                CommunityChest,
                Street {
                    group: Brown,
                    price: 60,
                    base_rent: 4,
                    house_rent: [20, 60, 180, 320, 450],
                    house_cost: 50,
                },
                IncomeTax,
                Railroad { price: 200 },
                Street {
                    group: LightBlue,
                    price: 100,
                    base_rent: 6,
                    house_rent: [30, 90, 270, 400, 550],
                    house_cost: 50,
                },
                Chance,
                Street {
                    group: LightBlue,
                    price: 100,
                    base_rent: 6,
                    house_rent: [30, 90, 270, 400, 550],
                    house_cost: 50,
                },
                Street {
                    group: LightBlue,
                    price: 120,
                    base_rent: 8,
                    house_rent: [40, 100, 300, 450, 600],
                    house_cost: 50,
                },
                Jail,
                Street {
                    group: Pink,
                    price: 140,
                    base_rent: 10,
                    house_rent: [50, 150, 450, 625, 750],
                    house_cost: 100,
                },
                Utility { price: 150 },
                Street {
                    group: Pink,
                    price: 140,
                    base_rent: 10,
                    house_rent: [50, 150, 450, 625, 750],
                    house_cost: 100,
                },
                Street {
                    group: Pink,
                    price: 160,
                    base_rent: 12,
                    house_rent: [60, 180, 500, 700, 900],
                    house_cost: 100,
                },
                Railroad { price: 200 },
                Street {
                    group: Orange,
                    price: 180,
                    base_rent: 14,
                    house_rent: [70, 200, 550, 750, 950],
                    house_cost: 100,
                },
                CommunityChest,
                Street {
                    group: Orange,
                    price: 180,
                    base_rent: 14,
                    house_rent: [70, 200, 550, 750, 950],
                    house_cost: 100,
                },
                Street {
                    group: Orange,
                    price: 200,
                    base_rent: 16,
                    house_rent: [80, 220, 600, 800, 1000],
                    house_cost: 100,
                },
                FreeParking,
                Street {
                    group: Red,
                    price: 220,
                    base_rent: 18,
                    house_rent: [90, 250, 700, 875, 1050],
                    house_cost: 150,
                },
                Chance,
                Street {
                    group: Red,
                    price: 220,
                    base_rent: 18,
                    house_rent: [90, 250, 700, 875, 1050],
                    house_cost: 150,
                },
                Street {
                    group: Red,
                    price: 240,
                    base_rent: 20,
                    house_rent: [100, 300, 750, 925, 1100],
                    house_cost: 150,
                },
                Railroad { price: 200 },
                Street {
                    group: Yellow,
                    price: 260,
                    base_rent: 22,
                    house_rent: [110, 330, 800, 975, 1150],
                    house_cost: 150,
                },
                Street {
                    group: Yellow,
                    price: 260,
                    base_rent: 22,
                    house_rent: [110, 330, 800, 975, 1150],
                    house_cost: 150,
                },
                Utility { price: 150 },
                Street {
                    group: Yellow,
                    price: 280,
                    base_rent: 24,
                    house_rent: [120, 360, 850, 1025, 1200],
                    house_cost: 150,
                },
                GoToJail,
                Street {
                    group: Green,
                    price: 300,
                    base_rent: 26,
                    house_rent: [130, 390, 900, 1100, 1275],
                    house_cost: 200,
                },
                Street {
                    group: Green,
                    price: 300,
                    base_rent: 26,
                    house_rent: [130, 390, 900, 1100, 1275],
                    house_cost: 200,
                },
                CommunityChest,
                Street {
                    group: Green,
                    price: 320,
                    base_rent: 28,
                    house_rent: [150, 450, 1000, 1200, 1400],
                    house_cost: 200,
                },
                Railroad { price: 200 },
                Chance,
                Street {
                    group: DarkBlue,
                    price: 350,
                    base_rent: 35,
                    house_rent: [175, 500, 1100, 1300, 1500],
                    house_cost: 200,
                },
                LuxuryTax,
                Street {
                    group: DarkBlue,
                    price: 400,
                    base_rent: 50,
                    house_rent: [200, 600, 1400, 1700, 2000],
                    house_cost: 200,
                },
            ],
        }
    }

    pub fn space(&self, index: usize) -> SpaceKind {
        self.spaces[index]
    }

    /// Every board space belonging to the given color group, in board order.
    pub fn group_members(&self, group: ColorGroup) -> impl Iterator<Item = usize> + '_ {
        GROUP_MEMBERS[group as usize].iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The precomputed `GROUP_MEMBERS` table must stay in sync with the board
    /// layout (and with the `ColorGroup` variant order it is indexed by).
    #[test]
    fn group_members_match_the_board() {
        let board = Board::standard();
        for (i, group) in ColorGroup::ALL.into_iter().enumerate() {
            assert_eq!(
                group as usize, i,
                "ColorGroup order must match GROUP_MEMBERS row order"
            );
            let scanned: Vec<usize> = (0..BOARD_SIZE)
                .filter(
                    |&s| matches!(board.space(s), SpaceKind::Street { group: g, .. } if g == group),
                )
                .collect();
            assert_eq!(
                board.group_members(group).collect::<Vec<_>>(),
                scanned,
                "{group:?}"
            );
        }
    }
}
