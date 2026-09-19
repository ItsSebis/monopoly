//! Static data for the standard 40-space Monopoly board.

pub const BOARD_SIZE: usize = 40;
pub const JAIL_SPACE: usize = 10;
pub const RAILROAD_SPACES: [usize; 4] = [5, 15, 25, 35];
pub const UTILITY_SPACES: [usize; 2] = [12, 28];

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
    Street { group: ColorGroup, price: u32, base_rent: u32 },
    Railroad { price: u32 },
    Utility { price: u32 },
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
        matches!(self, SpaceKind::Street { .. } | SpaceKind::Railroad { .. } | SpaceKind::Utility { .. })
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
                Street { group: Brown, price: 60, base_rent: 2 },
                CommunityChest,
                Street { group: Brown, price: 60, base_rent: 4 },
                IncomeTax,
                Railroad { price: 200 },
                Street { group: LightBlue, price: 100, base_rent: 6 },
                Chance,
                Street { group: LightBlue, price: 100, base_rent: 6 },
                Street { group: LightBlue, price: 120, base_rent: 8 },
                Jail,
                Street { group: Pink, price: 140, base_rent: 10 },
                Utility { price: 150 },
                Street { group: Pink, price: 140, base_rent: 10 },
                Street { group: Pink, price: 160, base_rent: 12 },
                Railroad { price: 200 },
                Street { group: Orange, price: 180, base_rent: 14 },
                CommunityChest,
                Street { group: Orange, price: 180, base_rent: 14 },
                Street { group: Orange, price: 200, base_rent: 16 },
                FreeParking,
                Street { group: Red, price: 220, base_rent: 18 },
                Chance,
                Street { group: Red, price: 220, base_rent: 18 },
                Street { group: Red, price: 240, base_rent: 20 },
                Railroad { price: 200 },
                Street { group: Yellow, price: 260, base_rent: 22 },
                Street { group: Yellow, price: 260, base_rent: 22 },
                Utility { price: 150 },
                Street { group: Yellow, price: 280, base_rent: 24 },
                GoToJail,
                Street { group: Green, price: 300, base_rent: 26 },
                Street { group: Green, price: 300, base_rent: 26 },
                CommunityChest,
                Street { group: Green, price: 320, base_rent: 28 },
                Railroad { price: 200 },
                Chance,
                Street { group: DarkBlue, price: 350, base_rent: 35 },
                LuxuryTax,
                Street { group: DarkBlue, price: 400, base_rent: 50 },
            ],
        }
    }

    pub fn space(&self, index: usize) -> SpaceKind {
        self.spaces[index]
    }

    /// Every board space belonging to the given color group, in board order.
    pub fn group_members(&self, group: ColorGroup) -> impl Iterator<Item = usize> + '_ {
        self.spaces.iter().enumerate().filter_map(move |(i, s)| match s {
            SpaceKind::Street { group: g, .. } if *g == group => Some(i),
            _ => None,
        })
    }
}
