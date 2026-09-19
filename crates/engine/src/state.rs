use crate::board::{Board, ColorGroup, BOARD_SIZE, RAILROAD_SPACES, UTILITY_SPACES};
use crate::cards::DeckKind;
use crate::rules::RuleSet;
use serde::Serialize;

/// The bank's starting supply of houses and hotels — a fixed scarcity that's
/// part of the real game's strategy, not configurable (see
/// docs/game-rules.md's Building houses and hotels section).
pub const STARTING_HOUSES: u8 = 32;
pub const STARTING_HOTELS: u8 = 12;

#[derive(Debug, Clone, Serialize)]
pub struct PlayerState {
    pub name: String,
    /// Signed so a payment can be checked/settled in one step before the
    /// bankruptcy check clamps it back to a sane value.
    pub cash: i64,
    pub position: usize,
    pub in_jail: bool,
    /// Number of failed roll-for-doubles attempts so far this jail stay
    /// (0-2). On the third attempt (`jail_turns == 2`), a failed roll forces
    /// an immediate pay-and-move instead of another wait — the official
    /// 3-turn cap.
    pub jail_turns: u8,
    pub bankrupt: bool,
    /// Which deck(s) a held "Get Out of Jail Free" card should return to
    /// once used or transferred — usually empty, occasionally one entry,
    /// rarely both.
    pub goojf_cards: Vec<DeckKind>,
}

impl PlayerState {
    fn new(name: String, starting_cash: u32) -> Self {
        PlayerState {
            name,
            cash: starting_cash as i64,
            position: 0,
            in_jail: false,
            jail_turns: 0,
            bankrupt: false,
            goojf_cards: Vec::new(),
        }
    }
}

/// A board space's ownable state. Unused (left default) on spaces that
/// aren't ownable.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct PropertyState {
    pub owner: Option<usize>,
    /// 0 = unimproved, 1-4 = that many houses, 5 = a hotel. Only meaningful
    /// for streets.
    pub houses: u8,
    pub mortgaged: bool,
}

/// Live game state. Property state is a `Vec` indexed directly by board
/// space (always exactly `BOARD_SIZE` long) rather than a hash map — the
/// board never changes size, so a direct index is both simpler and faster
/// than hashing for the every-turn ownership lookups the turn loop does.
#[derive(Debug, Clone, Serialize)]
pub struct GameState {
    pub turn: u32,
    pub current_player: usize,
    pub players: Vec<PlayerState>,
    pub properties: Vec<PropertyState>,
    pub bank_houses_remaining: u8,
    pub bank_hotels_remaining: u8,
    /// Accumulates payments to the bank when `RuleSet.free_parking_pot` is
    /// enabled; always 0 otherwise (see docs/game-rules.md's Free Parking
    /// section).
    pub free_parking_pot: u32,
}

impl GameState {
    pub(crate) fn new(rules: &RuleSet, names: &[String]) -> Self {
        GameState {
            turn: 0,
            current_player: 0,
            players: names
                .iter()
                .map(|n| PlayerState::new(n.clone(), rules.starting_cash))
                .collect(),
            properties: vec![PropertyState::default(); BOARD_SIZE],
            bank_houses_remaining: STARTING_HOUSES,
            bank_hotels_remaining: STARTING_HOTELS,
            free_parking_pot: 0,
        }
    }

    pub fn active_player_count(&self) -> usize {
        self.players.iter().filter(|p| !p.bankrupt).count()
    }
}

/// Read-only view handed to strategies: everything a human player could see
/// on the physical board, nothing they couldn't (e.g. no direct mutation).
pub struct GameView<'a> {
    pub board: &'a Board,
    pub rules: &'a RuleSet,
    pub state: &'a GameState,
}

impl GameView<'_> {
    pub fn player(&self, index: usize) -> &PlayerState {
        &self.state.players[index]
    }

    pub fn property(&self, space: usize) -> &PropertyState {
        &self.state.properties[space]
    }

    pub fn owner_of(&self, space: usize) -> Option<usize> {
        self.state.properties[space].owner
    }

    /// Whether `player` owns every street in `group` (the monopoly bonus
    /// applies to rent regardless of whether any houses have been built).
    pub fn owns_full_group(&self, player: usize, group: ColorGroup) -> bool {
        self.board
            .group_members(group)
            .all(|space| self.state.properties[space].owner == Some(player))
    }

    /// Every space in `group`, alongside its current house count — the shape
    /// `building.rs`'s pure validation functions want.
    pub fn group_house_counts(&self, group: ColorGroup) -> Vec<u8> {
        self.board
            .group_members(group)
            .map(|space| self.state.properties[space].houses)
            .collect()
    }

    pub fn owned_railroad_count(&self, player: usize) -> usize {
        RAILROAD_SPACES
            .iter()
            .filter(|&&s| self.state.properties[s].owner == Some(player))
            .count()
    }

    pub fn owned_utility_count(&self, player: usize) -> usize {
        UTILITY_SPACES
            .iter()
            .filter(|&&s| self.state.properties[s].owner == Some(player))
            .count()
    }

    /// Cash on hand plus the purchase price of every property owned
    /// (undiscounted for mortgage/building state — matching the official
    /// income-tax rule, which uses list price regardless of improvements).
    pub fn net_worth(&self, player: usize) -> u32 {
        let cash = self.state.players[player].cash.max(0) as u32;
        let properties: u32 = (0..BOARD_SIZE)
            .filter(|&s| self.state.properties[s].owner == Some(player))
            .filter_map(|s| self.board.space(s).price())
            .sum();
        cash + properties
    }
}
