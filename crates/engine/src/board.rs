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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceKind {
    Go,
    Street {
        group: ColorGroup,
        price: u32,
        base_rent: u32,
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
    /// Whether this space can ever be owned by a player.
    pub fn is_ownable(&self) -> bool {
        matches!(
            self,
            SpaceKind::Street { .. } | SpaceKind::Railroad { .. } | SpaceKind::Utility { .. }
        )
    }

    /// The bank purchase price, for ownable spaces.
    pub fn price(&self) -> Option<u32> {
        match self {
            SpaceKind::Street { price, .. } => Some(*price),
            SpaceKind::Railroad { price } => Some(*price),
            SpaceKind::Utility { price } => Some(*price),
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

        Board {
            spaces: [
                Go,
                Street {
                    group: Brown,
                    price: 60,
                    base_rent: 2,
                },
                CommunityChest,
                Street {
                    group: Brown,
                    price: 60,
                    base_rent: 4,
                },
                IncomeTax,
                Railroad { price: 200 },
                Street {
                    group: LightBlue,
                    price: 100,
                    base_rent: 6,
                },
                Chance,
                Street {
                    group: LightBlue,
                    price: 100,
                    base_rent: 6,
                },
                Street {
                    group: LightBlue,
                    price: 120,
                    base_rent: 8,
                },
                Jail,
                Street {
                    group: Pink,
                    price: 140,
                    base_rent: 10,
                },
                Utility { price: 150 },
                Street {
                    group: Pink,
                    price: 140,
                    base_rent: 10,
                },
                Street {
                    group: Pink,
                    price: 160,
                    base_rent: 12,
                },
                Railroad { price: 200 },
                Street {
                    group: Orange,
                    price: 180,
                    base_rent: 14,
                },
                CommunityChest,
                Street {
                    group: Orange,
                    price: 180,
                    base_rent: 14,
                },
                Street {
                    group: Orange,
                    price: 200,
                    base_rent: 16,
                },
                FreeParking,
                Street {
                    group: Red,
                    price: 220,
                    base_rent: 18,
                },
                Chance,
                Street {
                    group: Red,
                    price: 220,
                    base_rent: 18,
                },
                Street {
                    group: Red,
                    price: 240,
                    base_rent: 20,
                },
                Railroad { price: 200 },
                Street {
                    group: Yellow,
                    price: 260,
                    base_rent: 22,
                },
                Street {
                    group: Yellow,
                    price: 260,
                    base_rent: 22,
                },
                Utility { price: 150 },
                Street {
                    group: Yellow,
                    price: 280,
                    base_rent: 24,
                },
                GoToJail,
                Street {
                    group: Green,
                    price: 300,
                    base_rent: 26,
                },
                Street {
                    group: Green,
                    price: 300,
                    base_rent: 26,
                },
                CommunityChest,
                Street {
                    group: Green,
                    price: 320,
                    base_rent: 28,
                },
                Railroad { price: 200 },
                Chance,
                Street {
                    group: DarkBlue,
                    price: 350,
                    base_rent: 35,
                },
                LuxuryTax,
                Street {
                    group: DarkBlue,
                    price: 400,
                    base_rent: 50,
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
        let groups = [
            ColorGroup::Brown,
            ColorGroup::LightBlue,
            ColorGroup::Pink,
            ColorGroup::Orange,
            ColorGroup::Red,
            ColorGroup::Yellow,
            ColorGroup::Green,
            ColorGroup::DarkBlue,
        ];
        for (i, group) in groups.into_iter().enumerate() {
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
