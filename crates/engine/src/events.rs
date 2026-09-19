use serde::Serialize;

use crate::strategy::JailAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TaxKind {
    Income,
    Luxury,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JailReason {
    GoToJailSpace,
    ThreeDoubles,
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
    /// `Bankrupted`, the amount was never actually collected — Phase 1
    /// treats every bankruptcy as bankruptcy-to-bank (see docs/roadmap.md),
    /// so `to`/the tax payee never receives a partial payment either.
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
    Bankrupted,
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
