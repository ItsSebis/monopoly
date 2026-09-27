//! A 1:1 mirror of `Strategy`'s 7 hooks (`docs/simulation-engine.md#the-strategy-trait`),
//! reusing the engine's own (now-serializable, Phase 9) decision types rather
//! than hand-duplicating them. `PendingDecision` is what a session publishes
//! for the browser to poll; `DecisionAnswer` is what the browser posts back.
//!
//! The two enums are deliberately kept in lockstep, variant-for-variant —
//! `DecisionAnswer::answers` matches a `(DecisionAnswer, PendingDecision)`
//! pair directly rather than comparing string tags, so adding a variant to
//! one without the other is a compile error here instead of a silent gap.

use monopoly_engine::board::BOARD_SIZE;
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
    /// Whether `self` is a legal answer to `pending` — the same decision
    /// `kind`, matched structurally rather than via a hand-paired string tag
    /// on each side.
    pub fn answers(&self, pending: &PendingDecision) -> bool {
        matches!(
            (self, pending),
            (
                DecisionAnswer::Purchase { .. },
                PendingDecision::Purchase { .. }
            ) | (
                DecisionAnswer::JailAction { .. },
                PendingDecision::JailAction { .. }
            ) | (DecisionAnswer::Build { .. }, PendingDecision::Build { .. })
                | (
                    DecisionAnswer::Mortgage { .. },
                    PendingDecision::Mortgage { .. }
                )
                | (
                    DecisionAnswer::AuctionBid { .. },
                    PendingDecision::AuctionBid { .. }
                )
                | (
                    DecisionAnswer::TradeProposal { .. },
                    PendingDecision::TradeProposal { .. }
                )
                | (
                    DecisionAnswer::TradeResponse { .. },
                    PendingDecision::TradeResponse { .. }
                )
        )
    }

    /// Rejects a board-space index that's out of range before this answer
    /// ever reaches the session's channel. The engine itself already treats
    /// an out-of-range `Build`/`Mortgage`/`TradeProposal` action as a safe
    /// no-op (`Game::try_build`/`try_sell_house`/`try_mortgage`/
    /// `trade_is_valid`, all bounds-checked) — this is defense in depth for
    /// a clearer 400 instead of a silently-ignored action, now that answers
    /// come from client JSON rather than only trusted built-in strategies.
    pub fn validate(&self) -> Result<(), String> {
        let check = |space: usize| -> Result<(), String> {
            if space < BOARD_SIZE {
                Ok(())
            } else {
                Err(format!(
                    "space index {space} is out of range (board has {BOARD_SIZE} spaces)"
                ))
            }
        };
        match self {
            DecisionAnswer::Build { actions } => actions.iter().try_for_each(|action| {
                let (BuildAction::Build(space) | BuildAction::SellHouse(space)) = *action;
                check(space)
            }),
            DecisionAnswer::Mortgage { actions } => actions.iter().try_for_each(|action| {
                let (MortgageAction::Mortgage(space) | MortgageAction::SellHouse(space)) = *action;
                check(space)
            }),
            DecisionAnswer::TradeProposal { offer: Some(offer) } => offer
                .offered_properties
                .iter()
                .chain(&offer.requested_properties)
                .try_for_each(|&space| check(space)),
            _ => Ok(()),
        }
    }
}
