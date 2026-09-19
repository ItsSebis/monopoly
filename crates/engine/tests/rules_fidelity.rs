//! End-to-end, public-API-only tests. Finer-grained rule tests (rent/tax
//! math, the jail/doubles state machine, bankruptcy) live as `#[cfg(test)]`
//! unit tests alongside the code they cover — see docs/testing-and-validation.md.

use monopoly_engine::{Game, PlayerConfig, RuleSet};

fn players(strategies: &[(&str, &str)]) -> Vec<PlayerConfig> {
    strategies.iter().map(|(name, strategy)| PlayerConfig { name: (*name).into(), strategy: (*strategy).into() }).collect()
}

#[test]
fn a_game_always_terminates_with_a_winner() {
    let config = players(&[("Alice", "buy_good"), ("Bob", "buy_all"), ("Cara", "buy_bad"), ("Dan", "buy_none")]);
    let mut game = Game::new(RuleSet::default(), &config, 42).unwrap();
    let result = game.run_to_completion();
    assert!(result.winner.is_some(), "expected a sole survivor well within the internal safety cap");
}

#[test]
fn the_same_seed_always_produces_the_same_game() {
    let config = players(&[("A", "buy_good"), ("B", "buy_all"), ("C", "buy_bad")]);
    let run = |seed| {
        let mut game = Game::new(RuleSet::default(), &config, seed).unwrap();
        game.run_to_completion()
    };
    let a = run(1234);
    let b = run(1234);
    assert_eq!(a.turns, b.turns);
    assert_eq!(a.winner, b.winner);
    assert_eq!(serde_json::to_string(&a.events).unwrap(), serde_json::to_string(&b.events).unwrap());
}

#[test]
fn different_seeds_can_produce_different_games() {
    let config = players(&[("A", "buy_good"), ("B", "buy_all")]);
    let run = |seed| {
        let mut game = Game::new(RuleSet::default(), &config, seed).unwrap();
        game.run_to_completion()
    };
    let a = run(1);
    let b = run(2);
    assert_ne!(
        serde_json::to_string(&a.events).unwrap(),
        serde_json::to_string(&b.events).unwrap(),
        "different seeds landing on the exact same sequence of events would indicate the RNG isn't actually being used"
    );
}

#[test]
fn unknown_strategy_id_is_a_clear_error_not_a_panic() {
    let config = players(&[("X", "nonexistent")]);
    match Game::new(RuleSet::default(), &config, 1) {
        Ok(_) => panic!("expected an error for an unregistered strategy id"),
        Err(err) => assert_eq!(err.0, "nonexistent"),
    }
}
