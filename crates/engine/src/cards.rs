//! The Chance and Community Chest decks: the classic 16+16 US-edition set
//! (see docs/game-rules.md). Pure data — no dependency on `Game` or
//! `GameState`; `Game::apply_card_effect` is what actually applies these.
//!
//! Cards with an identical mechanical effect (many of these are just "collect
//! a flat amount from the bank" with different flavor text — e.g. "bank
//! error in your favor" and "you inherit $100" are indistinguishable to the
//! simulation) share one `CardEffect` variant rather than getting a variant
//! each; the event log records the effect and amount, not the flavor text.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use serde::Serialize;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DeckKind {
    Chance,
    CommunityChest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CardEffect {
    /// Move directly to a space (paying GO salary as normal if this passes
    /// or lands on GO) — covers every "advance to <named space>" card.
    AdvanceTo(usize),
    /// If unowned, may buy; if owned, pay the owner double their normal rent.
    AdvanceToNearestRailroad,
    /// If unowned, may buy; if owned, pay the owner 10x the dice roll.
    AdvanceToNearestUtility,
    CollectFromBank(u32),
    PayBank(u32),
    CollectFromEachPlayer(u32),
    PayEachPlayer(u32),
    /// "Make general repairs" / "assessed for street repairs": pay the bank
    /// per house and per hotel owned, across all properties.
    PropertyRepairAssessment {
        per_house: u32,
        per_hotel: u32,
    },
    GoBackThreeSpaces,
    GoToJail,
    GetOutOfJailFree,
}

pub struct Deck {
    pub kind: DeckKind,
    cards: VecDeque<CardEffect>,
}

impl Deck {
    /// Draws the top card, returning it to the bottom — except
    /// `GetOutOfJailFree`, which the caller removes from circulation (it's
    /// held by a player) and must return later via `return_card`.
    pub fn draw(&mut self) -> CardEffect {
        let card = self
            .cards
            .pop_front()
            .expect("a standard deck always has cards in it");
        if card != CardEffect::GetOutOfJailFree {
            self.cards.push_back(card);
        }
        card
    }

    /// Returns a previously-drawn `GetOutOfJailFree` card to the bottom of
    /// this deck, once it's used or its holder is bankrupted to the bank.
    pub fn return_get_out_of_jail_free_card(&mut self) {
        self.cards.push_back(CardEffect::GetOutOfJailFree);
    }

    /// Cards currently in the deck (a held `GetOutOfJailFree` card is out of
    /// circulation and not counted until it's returned).
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
}

/// The full standard board space indices these cards name. Chance and
/// Community Chest sit at spaces 5 (Reading Railroad, "take a trip"), 11
/// (St. Charles Place), and 24 (Illinois Avenue) — see docs/game-rules.md's
/// board layout.
mod named_spaces {
    pub const GO: usize = 0;
    pub const READING_RAILROAD: usize = 5;
    pub const ST_CHARLES_PLACE: usize = 11;
    pub const ILLINOIS_AVENUE: usize = 24;
}

fn chance_cards() -> Vec<CardEffect> {
    use named_spaces::*;
    use CardEffect::*;
    vec![
        AdvanceTo(GO),
        AdvanceTo(ILLINOIS_AVENUE),
        AdvanceTo(ST_CHARLES_PLACE),
        AdvanceToNearestUtility,
        AdvanceToNearestRailroad,
        AdvanceToNearestRailroad, // this card really does appear twice in the official deck
        CollectFromBank(50),      // bank pays you dividend
        GetOutOfJailFree,
        GoBackThreeSpaces,
        GoToJail,
        PropertyRepairAssessment {
            per_house: 25,
            per_hotel: 100,
        }, // general repairs
        PayBank(15), // speeding fine
        AdvanceTo(READING_RAILROAD),
        PayEachPlayer(50),    // chairman of the board
        CollectFromBank(150), // building loan matures
        CollectFromBank(100), // crossword competition
    ]
}

fn community_chest_cards() -> Vec<CardEffect> {
    use named_spaces::GO;
    use CardEffect::*;
    vec![
        AdvanceTo(GO),
        CollectFromBank(200), // bank error in your favor
        PayBank(50),          // doctor's fee
        CollectFromBank(50),  // sale of stock
        GetOutOfJailFree,
        GoToJail,
        CollectFromBank(100),      // holiday fund matures
        CollectFromBank(20),       // income tax refund
        CollectFromEachPlayer(10), // it's your birthday
        CollectFromBank(100),      // life insurance matures
        PayBank(100),              // hospital fees
        PayBank(150),              // school fees
        CollectFromBank(25),       // consultancy fee
        PropertyRepairAssessment {
            per_house: 40,
            per_hotel: 115,
        }, // street repairs
        CollectFromBank(10),       // beauty contest
        CollectFromBank(100),      // inherit
    ]
}

/// Builds and shuffles both decks once, using the game's own seeded RNG —
/// so, like every other random element, deck order is a pure function of the
/// game's seed (see docs/architecture.md's determinism guarantee).
pub fn standard_decks(rng: &mut StdRng) -> (Deck, Deck) {
    let mut chance = chance_cards();
    let mut community_chest = community_chest_cards();
    chance.shuffle(rng);
    community_chest.shuffle(rng);
    (
        Deck {
            kind: DeckKind::Chance,
            cards: chance.into(),
        },
        Deck {
            kind: DeckKind::CommunityChest,
            cards: community_chest.into(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_decks_have_sixteen_cards() {
        assert_eq!(chance_cards().len(), 16);
        assert_eq!(community_chest_cards().len(), 16);
    }

    #[test]
    fn each_deck_has_exactly_one_get_out_of_jail_free_card() {
        for cards in [chance_cards(), community_chest_cards()] {
            let count = cards
                .iter()
                .filter(|c| **c == CardEffect::GetOutOfJailFree)
                .count();
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn drawing_cycles_through_the_deck_except_get_out_of_jail_free() {
        let mut rng = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(1);
        let (mut chance, _) = standard_decks(&mut rng);
        let first_pass: Vec<CardEffect> = (0..16).map(|_| chance.draw()).collect();
        let non_goojf_count = first_pass
            .iter()
            .filter(|c| **c != CardEffect::GetOutOfJailFree)
            .count();
        // Every non-GOOJF card returns to the bottom, so a second full pass
        // reproduces the same 15 cards (GOOJF is missing until returned).
        let second_pass: Vec<CardEffect> = (0..non_goojf_count).map(|_| chance.draw()).collect();
        assert_eq!(
            first_pass
                .into_iter()
                .filter(|c| *c != CardEffect::GetOutOfJailFree)
                .collect::<Vec<_>>(),
            second_pass
        );
    }
}
