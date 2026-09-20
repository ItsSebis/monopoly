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
pub mod run_record;
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
pub use game::{ConfigError, Game, GameResult, SAFETY_MAX_TURNS};
pub use rules::{IncomeTaxMode, RuleSet};
pub use run_record::{
    build_batch_run_record, build_single_run_record, derive_batch_seeds, BatchRunRecord,
    SingleRunRecord,
};
pub use stats::{compute_stats, PerGameStats};
pub use strategies::STRATEGY_IDS;
pub use strategy::{BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy, TradeOffer};
