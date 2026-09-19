//! The Monopoly simulation engine: board data, rules, the turn state
//! machine, and the `Strategy` trait. No I/O — see `docs/architecture.md`.

pub mod board;
pub mod config;
pub mod events;
pub mod game;
pub mod rent;
pub mod rules;
pub mod state;
pub mod strategies;
pub mod strategy;
pub mod tax;

pub use config::{GameConfig, PlayerConfig};
pub use events::{Event, EventEnvelope};
pub use game::{Game, GameResult, UnknownStrategy};
pub use rules::{IncomeTaxMode, RuleSet};
pub use strategy::{JailAction, PurchaseOffer, Strategy};
