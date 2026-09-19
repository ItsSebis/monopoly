//! The Monopoly simulation engine: board data, rules, the turn state
//! machine, and the `Strategy` trait. No I/O — see `docs/architecture.md`.

pub mod batch;
pub mod board;
pub mod building;
pub mod cards;
pub mod config;
pub mod events;
pub mod game;
pub mod rent;
pub mod rules;
pub mod state;
pub mod stats;
pub mod strategies;
pub mod strategy;
pub mod tax;

pub use batch::{
    run_batch, AggregateStats, BatchGameSummary, BatchResult, DistributionSummary, HeadToHead,
};
pub use board::Board;
pub use cards::{CardEffect, DeckKind};
pub use config::{GameConfig, PlayerConfig};
pub use events::{Event, EventEnvelope};
pub use game::{ConfigError, Game, GameResult};
pub use rules::{IncomeTaxMode, RuleSet};
pub use stats::{compute_stats, PerGameStats};
pub use strategy::{BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy};
