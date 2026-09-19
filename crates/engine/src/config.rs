use serde::{Deserialize, Serialize};

use crate::rules::RuleSet;

/// One player's setup for a game. Strategy-specific parameters (e.g. a
/// custom cash reserve) aren't supported in Phase 1 since no built-in
/// strategy takes any yet — see docs/player-strategies.md's "Custom
/// strategies" section for when this grows a `strategy_params` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerConfig {
    pub name: String,
    pub strategy: String,
}

/// The full input to a game: `RuleSet` plus the players, matching
/// docs/headless-cli.md's config file shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    #[serde(default)]
    pub rules: RuleSet,
    pub players: Vec<PlayerConfig>,
}
