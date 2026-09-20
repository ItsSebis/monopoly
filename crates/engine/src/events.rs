use serde::Serialize;

use crate::cards::{CardEffect, DeckKind};
use crate::strategy::JailAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TaxKind {
    Income,
    Luxury,
    /// A Chance/Community Chest repair assessment (`CardEffect::PropertyRepairAssessment`).
    /// The `CardDrawn` event for that card only carries its per-house/per-hotel
    /// rates, not the total owed — this is where the actual amount charged
    /// is recorded.
    Repair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JailReason {
    GoToJailSpace,
    ThreeDoubles,
    Card,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum Event {
    RollDice {
        dice: (u8, u8),
    },
    Move {
        from: usize,
        to: usize,
    },
    PassGo,
    PropertyOffered {
        space: usize,
        price: u32,
    },
    PurchaseDecision {
        space: usize,
        bought: bool,
    },
    /// `amount` is what was owed. If this player's very next event is
    /// `Bankrupted`, the amount was never actually collected in full — see
    /// `Bankrupted`'s `payee` for where whatever they *could* raise went.
    RentPaid {
        to: usize,
        amount: u32,
        space: usize,
    },
    TaxPaid {
        amount: u32,
        kind: TaxKind,
    },
    JailEntered {
        reason: JailReason,
    },
    /// `forced` is true only for the mandatory pay-and-move after a failed
    /// third jail-escape attempt; a voluntarily chosen `PayFine` (on any
    /// attempt) is `forced: false`.
    JailDecision {
        action: JailAction,
        forced: bool,
    },
    JailExited,
    /// A drawn "Get Out of Jail Free" card is used automatically, not via a
    /// `JailDecision` — see `Strategy`'s doc comment for why.
    UsedGetOutOfJailFreeCard,
    HouseBuilt {
        space: usize,
    },
    HouseSold {
        space: usize,
    },
    /// Mortgaging is a one-way action in Phase 2 — no strategy hook ever
    /// chooses to unmortgage (see docs/player-strategies.md), so there's no
    /// corresponding `Unmortgaged` event yet.
    Mortgaged {
        space: usize,
    },
    CardDrawn {
        deck: DeckKind,
        effect: CardEffect,
    },
    AuctionBid {
        player: usize,
        amount: Option<u32>,
    },
    /// `amount` is what the winner actually paid (the second-highest bid, or
    /// $1 with only one bidder — see docs/roadmap.md's Phase 2 auction
    /// design note), not necessarily their own bid.
    AuctionWon {
        player: usize,
        space: usize,
        amount: u32,
    },
    /// `payee`: `Some` for bankruptcy-to-player (who received the remaining
    /// properties and any "Get Out of Jail Free" cards), `None` for
    /// bankruptcy-to-bank (properties return to the unowned pool, cards
    /// return to their decks).
    Bankrupted {
        payee: Option<usize>,
    },
    /// A trade `player` proposed to `to` (see `Strategy::decide_trade`) that
    /// `to` accepted and the engine successfully applied. `player`'s side
    /// gave up `offered_properties`/`offered_cash` and received
    /// `requested_properties`/`requested_cash` in return.
    TradeExecuted {
        to: usize,
        offered_properties: Vec<usize>,
        offered_cash: u32,
        requested_properties: Vec<usize>,
        requested_cash: u32,
    },
    /// A trade `player` proposed to `to` that `to` declined (or that failed
    /// re-validation - see `Game::maybe_trade`). No state changed.
    TradeDeclined {
        to: usize,
    },
    GameEnded {
        winner: Option<usize>,
        turns: u32,
    },
}

/// One entry in the full event log: the event plus enough context (which
/// turn, which player, and a global sequence number) to reconstruct ordering
/// and drive both playback and statistics from the same stream.
#[derive(Debug, Clone, Serialize)]
pub struct EventEnvelope {
    pub turn: u32,
    pub player: usize,
    pub seq: u64,
    pub event: Event,
}
