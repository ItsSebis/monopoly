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
    RollDice { dice: (u8, u8) },
    Move { from: usize, to: usize },
    PassGo,
    PropertyOffered { space: usize, price: u32 },
    PurchaseDecision { space: usize, bought: bool },
    RentPaid { to: usize, amount: u32, space: usize },
    TaxPaid { amount: u32, kind: TaxKind },
    JailEntered { reason: JailReason },
    JailDecision { action: JailAction, forced: bool },
    JailExited,
    Bankrupted,
    GameEnded { winner: Option<usize>, turns: u32 },
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
