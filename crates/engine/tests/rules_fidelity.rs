//! End-to-end, public-API-only tests. Finer-grained rule tests (rent/tax
//! math, the jail/doubles state machine, bankruptcy) live as `#[cfg(test)]`
//! unit tests alongside the code they cover — see docs/testing-and-validation.md.

use monopoly_engine::{ConfigError, Game, GameResult, PlayerConfig, RuleSet};

fn players(strategies: &[(&str, &str)]) -> Vec<PlayerConfig> {
    strategies
        .iter()
        .map(|(name, strategy)| PlayerConfig {
            name: (*name).into(),
            strategy: (*strategy).into(),
        })
        .collect()
}

fn run(players: &[PlayerConfig], seed: u64) -> GameResult {
    let mut game = Game::new(RuleSet::default(), players, seed).unwrap();
    game.run_to_completion()
}

/// Without house-building (Phase 1's scope), base rents are small enough
/// relative to GO salary that two cash-hoarding strategies can occasionally
/// out-earn each other indefinitely and never bankrupt (see
/// docs/game-rules.md's Bankruptcy section) — so a test asserting reliable
/// termination needs a matchup where one side can never earn anything.
///
/// Buy None never spends, so it can only go bankrupt on rent it can't pay —
/// but Buy All's thin $50 reserve (see docs/player-strategies.md) makes *it*
/// vulnerable to an unlucky early tax landing too, so this does not always
/// end with Buy All as the winner (confirmed: seed 6 ends with Buy All
/// bankrupting itself on tax while still owning nothing Buy None could have
/// been charged for). What this matchup does reliably do is end quickly,
/// since at least one side's cash can only trend downward.
#[test]
fn buy_all_vs_buy_none_reliably_terminates_quickly() {
    let config = players(&[("Greedy", "buy_all"), ("Passive", "buy_none")]);
    for seed in 0..20u64 {
        let result = run(&config, seed);
        assert!(
            result.winner.is_some(),
            "seed {seed}: expected a sole survivor well within the safety cap"
        );
        assert!(
            result.turns < 5_000,
            "seed {seed}: expected this matchup to resolve quickly, took {} turns",
            result.turns
        );
    }
}

#[test]
fn the_same_seed_always_produces_the_same_game() {
    let config = players(&[("A", "buy_good"), ("B", "buy_all"), ("C", "buy_bad")]);
    let a = run(&config, 1234);
    let b = run(&config, 1234);
    assert_eq!(a.turns, b.turns);
    assert_eq!(a.winner, b.winner);
    assert_eq!(
        serde_json::to_string(&a.events).unwrap(),
        serde_json::to_string(&b.events).unwrap()
    );
}

#[test]
fn different_seeds_can_produce_different_games() {
    let config = players(&[("A", "buy_good"), ("B", "buy_all")]);
    let a = run(&config, 1);
    let b = run(&config, 2);
    assert_ne!(
        serde_json::to_string(&a.events).unwrap(),
        serde_json::to_string(&b.events).unwrap(),
        "different seeds landing on the exact same sequence of events would indicate the RNG isn't actually being used"
    );
}

#[test]
fn unknown_strategy_id_is_a_clear_error_not_a_panic() {
    let config = players(&[("X", "nonexistent"), ("Y", "buy_all")]);
    match Game::new(RuleSet::default(), &config, 1) {
        Ok(_) => panic!("expected an error for an unregistered strategy id"),
        Err(err) => assert_eq!(err, ConfigError::UnknownStrategy("nonexistent".into())),
    }
}

#[test]
fn fewer_than_two_players_is_a_clear_error_not_a_panic() {
    let config = players(&[("Solo", "buy_all")]);
    match Game::new(RuleSet::default(), &config, 1) {
        Ok(_) => panic!("expected an error for a single-player config"),
        Err(err) => assert_eq!(err, ConfigError::NotEnoughPlayers(1)),
    }
}
