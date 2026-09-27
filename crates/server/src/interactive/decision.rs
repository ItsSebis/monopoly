//! A 1:1 mirror of `Strategy`'s 7 hooks (`docs/simulation-engine.md#the-strategy-trait`),
//! reusing the engine's own (now-serializable, Phase 9) decision types rather
//! than hand-duplicating them. `PendingDecision` is what a session publishes
//! for the browser to poll; `DecisionAnswer` is what the browser posts back.
//!
//! The two enums are deliberately kept in lockstep, variant-for-variant —
//! `kind()` on each returns the same string for the matching pair, which is
//! what lets the `POST /sessions/:id/decisions` handler reject a
//! mismatched-kind answer with a 409 rather than silently misinterpreting it.

use monopoly_engine::{BuildAction, JailAction, MortgageAction, PurchaseOffer, TradeOffer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum PendingDecision {
    Purchase { player: usize, offer: PurchaseOffer },
    JailAction { player: usize },
    Build { player: usize },
    Mortgage { player: usize, shortfall: u32 },
    AuctionBid { player: usize, space: usize },
    TradeProposal { player: usize },
    TradeResponse { player: usize, offer: TradeOffer },
}

impl PendingDecision {
    pub fn kind(&self) -> &'static str {
        match self {
            PendingDecision::Purchase { .. } => "Purchase",
            PendingDecision::JailAction { .. } => "JailAction",
            PendingDecision::Build { .. } => "Build",
            PendingDecision::Mortgage { .. } => "Mortgage",
            PendingDecision::AuctionBid { .. } => "AuctionBid",
            PendingDecision::TradeProposal { .. } => "TradeProposal",
            PendingDecision::TradeResponse { .. } => "TradeResponse",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum DecisionAnswer {
    Purchase { buy: bool },
    JailAction { action: JailAction },
    Build { actions: Vec<BuildAction> },
    Mortgage { actions: Vec<MortgageAction> },
    AuctionBid { amount: Option<u32> },
    TradeProposal { offer: Option<TradeOffer> },
    TradeResponse { accept: bool },
}

impl DecisionAnswer {
    pub fn kind(&self) -> &'static str {
        match self {
            DecisionAnswer::Purchase { .. } => "Purchase",
            DecisionAnswer::JailAction { .. } => "JailAction",
            DecisionAnswer::Build { .. } => "Build",
            DecisionAnswer::Mortgage { .. } => "Mortgage",
            DecisionAnswer::AuctionBid { .. } => "AuctionBid",
            DecisionAnswer::TradeProposal { .. } => "TradeProposal",
            DecisionAnswer::TradeResponse { .. } => "TradeResponse",
        }
    }
}
