use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;

use crate::board::{Board, SpaceKind, BOARD_SIZE, JAIL_SPACE};
use crate::config::PlayerConfig;
use crate::events::{Event, EventEnvelope, JailReason, TaxKind};
use crate::rules::RuleSet;
use crate::state::{GameState, GameView};
use crate::strategies::make_strategy;
use crate::strategy::{JailAction, PurchaseOffer, Strategy};

/// Internal safety valve against a non-terminating game — e.g. every player
/// running Buy None can in principle run for a very long time (see
/// docs/player-strategies.md). Not user-configurable; Phase 3's batch runner
/// introduces a documented, configurable `max_turns` for a different
/// purpose (bounding batch run cost).
const SAFETY_MAX_TURNS: u32 = 20_000;

pub struct Game {
    board: Board,
    rules: RuleSet,
    state: GameState,
    strategies: Vec<Box<dyn Strategy>>,
    rng: StdRng,
    seq: u64,
    log: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameResult {
    /// `None` if the game hit the internal safety cap without a single
    /// player remaining.
    pub winner: Option<usize>,
    pub turns: u32,
    pub events: Vec<EventEnvelope>,
    pub final_state: GameState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    UnknownStrategy(String),
    /// Monopoly needs at least 2 players; fewer is a config mistake, not a
    /// degenerate-but-valid game (a 0- or 1-player game would trivially
    /// "win" at turn 0 with no rolls ever made).
    NotEnoughPlayers(usize),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::UnknownStrategy(id) => write!(f, "unknown strategy id: {id}"),
            ConfigError::NotEnoughPlayers(n) => write!(f, "need at least 2 players, got {n}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Game {
    pub fn new(rules: RuleSet, players: &[PlayerConfig], seed: u64) -> Result<Self, ConfigError> {
        if players.len() < 2 {
            return Err(ConfigError::NotEnoughPlayers(players.len()));
        }
        let names: Vec<String> = players.iter().map(|p| p.name.clone()).collect();
        let strategies = players
            .iter()
            .map(|p| {
                make_strategy(&p.strategy)
                    .ok_or_else(|| ConfigError::UnknownStrategy(p.strategy.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let state = GameState::new(&rules, &names);
        Ok(Game {
            board: Board::standard(),
            rules,
            state,
            strategies,
            rng: StdRng::seed_from_u64(seed),
            seq: 0,
            log: Vec::new(),
        })
    }

    pub fn is_over(&self) -> bool {
        self.state.active_player_count() <= 1
    }

    /// The current game state, for inspecting the board between `step_turn`
    /// calls (e.g. Phase 5's live browser playback, which renders after
    /// every turn rather than only at the end).
    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// Runs turns until one player remains (or the internal safety cap is
    /// hit) and returns the full result.
    pub fn run_to_completion(&mut self) -> GameResult {
        while !self.is_over() && self.state.turn < SAFETY_MAX_TURNS {
            self.step_turn();
        }
        let winner = match self.state.active_player_count() {
            1 => self.state.players.iter().position(|p| !p.bankrupt),
            _ => None,
        };
        let turns = self.state.turn;
        // Attribute the closing event to the winner when there is one: if the
        // game ended by the *current* player's own bankruptcy, `current_player`
        // still points at that now-removed player (`finish_turn` stops
        // advancing once `is_over()`), which would otherwise mislabel the
        // final envelope.
        let closing_player = winner.unwrap_or(self.state.current_player);
        self.record(closing_player, Event::GameEnded { winner, turns });
        // The log is moved out rather than cloned: it reaches ~65k envelopes
        // (~3 MB) in a long game, and copying that once per game is pure
        // overhead for a batch runner that only wants the finished log.
        GameResult {
            winner,
            turns,
            events: std::mem::take(&mut self.log),
            final_state: self.state.clone(),
        }
    }

    fn record(&mut self, player: usize, event: Event) {
        self.seq += 1;
        self.log.push(EventEnvelope {
            turn: self.state.turn,
            player,
            seq: self.seq,
            event,
        });
    }

    fn view(&self) -> GameView<'_> {
        GameView {
            board: &self.board,
            rules: &self.rules,
            state: &self.state,
        }
    }

    /// A player's strategy alongside a view of the rest of the game. Split
    /// field-by-field rather than going through `self.view()`, which would
    /// borrow all of `self` — including the strategy being called.
    fn strategy_and_view(&mut self, player: usize) -> (&mut dyn Strategy, GameView<'_>) {
        let view = GameView {
            board: &self.board,
            rules: &self.rules,
            state: &self.state,
        };
        (self.strategies[player].as_mut(), view)
    }

    /// Advances the game by exactly one player's turn (which may itself
    /// include multiple dice rolls on doubles) and returns the events that
    /// turn produced. See docs/simulation-engine.md's turn state machine.
    /// `run_to_completion` drives this in a loop; later phases (e.g. the
    /// browser's live playback in Phase 5) can call it one turn at a time.
    pub fn step_turn(&mut self) -> &[EventEnvelope] {
        let start = self.log.len();
        self.state.turn += 1;
        let player = self.state.current_player;

        if !self.state.players[player].bankrupt {
            let takes_normal_turn = if self.state.players[player].in_jail {
                self.resolve_jail_start_of_turn(player)
                    && !self.state.players[player].bankrupt
                    && !self.state.players[player].in_jail
            } else {
                true
            };
            if takes_normal_turn {
                self.run_dice_loop(player);
            }
        }

        self.finish_turn();
        &self.log[start..]
    }

    fn run_dice_loop(&mut self, player: usize) {
        let mut doubles_in_a_row = 0u8;
        loop {
            let (first, second) = self.roll_dice(player);
            let is_double = first == second;
            doubles_in_a_row = if is_double { doubles_in_a_row + 1 } else { 0 };
            if doubles_in_a_row == 3 {
                self.send_to_jail(player, JailReason::ThreeDoubles);
                break;
            }
            self.move_player(player, first + second);
            self.resolve_landing(player, first + second);
            let player_state = &self.state.players[player];
            if player_state.bankrupt || player_state.in_jail || !is_double {
                break;
            }
        }
    }

    fn finish_turn(&mut self) {
        if self.is_over() {
            return;
        }
        loop {
            self.state.current_player = (self.state.current_player + 1) % self.state.players.len();
            if !self.state.players[self.state.current_player].bankrupt {
                break;
            }
        }
    }

    fn roll_dice(&mut self, player: usize) -> (u8, u8) {
        let dice = (self.rng.gen_range(1..=6), self.rng.gen_range(1..=6));
        self.record(player, Event::RollDice { dice });
        dice
    }

    fn move_player(&mut self, player: usize, spaces: u8) {
        let old = self.state.players[player].position;
        let new = (old + spaces as usize) % BOARD_SIZE;
        self.state.players[player].position = new;
        self.record(player, Event::Move { from: old, to: new });
        if new <= old {
            self.pay_from_bank(player, self.rules.go_salary);
            self.record(player, Event::PassGo);
        }
    }

    fn pay_from_bank(&mut self, player: usize, amount: u32) {
        self.state.players[player].cash += amount as i64;
    }

    /// Deducts `amount` from `player`, crediting `payee` if given (otherwise
    /// the payment simply leaves play, as with tax). If the player can't
    /// cover it, they go bankrupt: Phase 1 only implements
    /// bankruptcy-to-bank (see docs/roadmap.md) — their properties return to
    /// the unowned pool regardless of who they owed, and the payment itself
    /// is never completed. Bankruptcy-to-player is a Phase 2 addition
    /// alongside mortgaging.
    fn charge(&mut self, player: usize, amount: u32, payee: Option<usize>) {
        if self.state.players[player].cash >= amount as i64 {
            self.state.players[player].cash -= amount as i64;
            if let Some(payee) = payee {
                self.state.players[payee].cash += amount as i64;
            }
        } else {
            self.bankrupt_player(player);
        }
    }

    fn bankrupt_player(&mut self, player: usize) {
        self.state.players[player].cash = 0;
        self.state.players[player].bankrupt = true;
        // A player bankrupted *while jailed* (e.g. by the forced fine on the
        // last jail turn) is out of the game, so leaving `in_jail`/`jail_turns`
        // set would leave the final state claiming a removed player is still
        // serving a sentence.
        self.state.players[player].in_jail = false;
        self.state.players[player].jail_turns = 0;
        for owner in self.state.owners.iter_mut() {
            if *owner == Some(player) {
                *owner = None;
            }
        }
        self.record(player, Event::Bankrupted);
    }

    fn send_to_jail(&mut self, player: usize, reason: JailReason) {
        self.state.players[player].in_jail = true;
        self.state.players[player].jail_turns = 0;
        self.state.players[player].position = JAIL_SPACE;
        self.record(player, Event::JailEntered { reason });
    }

    fn exit_jail(&mut self, player: usize) {
        self.state.players[player].in_jail = false;
        self.state.players[player].jail_turns = 0;
        self.record(player, Event::JailExited);
    }

    fn exit_jail_and_move(&mut self, player: usize, dice_total: u8) {
        self.exit_jail(player);
        self.move_player(player, dice_total);
        self.resolve_landing(player, dice_total);
    }

    /// Pays the jail fine; returns false if that payment bankrupted the
    /// player (in which case they're out of the game, not just out of jail).
    fn pay_jail_fine(&mut self, player: usize) -> bool {
        self.charge(player, self.rules.jail_fine, None);
        !self.state.players[player].bankrupt
    }

    /// Resolves the jail decision at the start of a jailed player's turn.
    /// Official rules give a jailed player up to 3 turns to roll doubles; if
    /// the third roll also fails, they must pay the fine immediately and
    /// move using that same roll (see docs/game-rules.md's Jail section).
    /// Returns whether the player should go on to take a normal turn this
    /// same call (true after voluntarily paying the fine — they still need
    /// to roll and move; false in every case that already moved them this
    /// call, since neither a successful escape roll nor the forced
    /// third-attempt move grants the usual "doubles = go again" bonus).
    fn resolve_jail_start_of_turn(&mut self, player: usize) -> bool {
        let (strategy, view) = self.strategy_and_view(player);
        let action = strategy.decide_jail_action(&view, player);
        self.record(
            player,
            Event::JailDecision {
                action,
                forced: false,
            },
        );

        match action {
            JailAction::PayFine => {
                if !self.pay_jail_fine(player) {
                    return false;
                }
                self.exit_jail(player);
                true
            }
            JailAction::RollForDoubles => {
                let is_third_attempt = self.state.players[player].jail_turns >= 2;
                let (first, second) = self.roll_dice(player);
                if first == second {
                    self.exit_jail_and_move(player, first + second);
                } else if is_third_attempt {
                    self.record(
                        player,
                        Event::JailDecision {
                            action: JailAction::PayFine,
                            forced: true,
                        },
                    );
                    if !self.pay_jail_fine(player) {
                        return false;
                    }
                    self.exit_jail_and_move(player, first + second);
                } else {
                    self.state.players[player].jail_turns += 1;
                }
                false
            }
        }
    }

    fn resolve_landing(&mut self, player: usize, dice_total: u8) {
        let space = self.state.players[player].position;
        match self.board.space(space) {
            SpaceKind::Street { .. } | SpaceKind::Railroad { .. } | SpaceKind::Utility { .. } => {
                self.resolve_ownable_landing(player, space, dice_total);
            }
            SpaceKind::IncomeTax => {
                let amount = self.income_tax_due(player);
                self.charge_tax(player, amount, TaxKind::Income);
            }
            SpaceKind::LuxuryTax => self.charge_tax(player, self.rules.luxury_tax, TaxKind::Luxury),
            SpaceKind::GoToJail => self.send_to_jail(player, JailReason::GoToJailSpace),
            // No effect yet: Free Parking has no pot in the baseline rules,
            // Chance/Community Chest decks are a documented Phase 2 stub
            // (see docs/game-rules.md), and GO/Jail have no landing effect
            // beyond GO's salary (already handled in move_player) and
            // Jail's "just visiting".
            SpaceKind::Go
            | SpaceKind::FreeParking
            | SpaceKind::Chance
            | SpaceKind::CommunityChest
            | SpaceKind::Jail => {}
        }
    }

    fn charge_tax(&mut self, player: usize, amount: u32, kind: TaxKind) {
        self.charge(player, amount, None);
        self.record(player, Event::TaxPaid { amount, kind });
    }

    fn resolve_ownable_landing(&mut self, player: usize, space: usize, dice_total: u8) {
        match self.state.owners[space] {
            None => {
                let price = self
                    .board
                    .space(space)
                    .price()
                    .expect("ownable space has a price");
                self.record(player, Event::PropertyOffered { space, price });
                let offer = PurchaseOffer { space, price };
                let (strategy, view) = self.strategy_and_view(player);
                let wants_to_buy = strategy.decide_purchase(&view, player, &offer);
                let bought = wants_to_buy && self.state.players[player].cash >= price as i64;
                if bought {
                    self.state.players[player].cash -= price as i64;
                    self.state.owners[space] = Some(player);
                }
                self.record(player, Event::PurchaseDecision { space, bought });
            }
            Some(owner) if owner != player => {
                let amount = self.rent_due(space, owner, dice_total);
                self.charge(player, amount, Some(owner));
                self.record(
                    player,
                    Event::RentPaid {
                        to: owner,
                        amount,
                        space,
                    },
                );
            }
            Some(_) => {}
        }
    }

    fn rent_due(&self, space: usize, owner: usize, dice_total: u8) -> u32 {
        let view = self.view();
        match self.board.space(space) {
            SpaceKind::Street {
                group, base_rent, ..
            } => crate::rent::street_rent(base_rent, view.owns_full_group(owner, group)),
            SpaceKind::Railroad { .. } => {
                crate::rent::railroad_rent(view.owned_railroad_count(owner))
            }
            SpaceKind::Utility { .. } => {
                crate::rent::utility_rent(view.owned_utility_count(owner), dice_total)
            }
            _ => unreachable!("rent is only charged on ownable spaces"),
        }
    }

    fn income_tax_due(&self, player: usize) -> u32 {
        crate::tax::income_tax_due(self.rules.income_tax_mode, self.view().net_worth(player))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PlayerConfig;

    fn two_players() -> Vec<PlayerConfig> {
        vec![
            PlayerConfig {
                name: "A".into(),
                strategy: "buy_good".into(),
            },
            PlayerConfig {
                name: "B".into(),
                strategy: "buy_good".into(),
            },
        ]
    }

    #[test]
    fn three_doubles_in_a_row_sends_the_player_to_jail_without_moving_the_third_roll() {
        let players = two_players();
        for seed in 0..50_000u64 {
            let mut game = Game::new(RuleSet::default(), &players, seed).unwrap();
            let events = game.step_turn();
            let doubles_rolled = events
                .iter()
                .filter(|e| matches!(e.event, Event::RollDice { dice } if dice.0 == dice.1))
                .count();
            let moves_made = events
                .iter()
                .filter(|e| matches!(e.event, Event::Move { .. }))
                .count();
            if doubles_rolled == 3 {
                assert!(
                    game.state.players[0].in_jail,
                    "seed {seed}: three doubles should send the player to jail"
                );
                assert_eq!(game.state.players[0].position, JAIL_SPACE);
                assert_eq!(
                    moves_made, 2,
                    "seed {seed}: the third (jailing) double must not move the player"
                );
                return;
            }
        }
        panic!("no seed among the first 50,000 rolled three doubles in a row on turn one");
    }

    #[test]
    fn charging_more_than_a_player_can_pay_bankrupts_them_and_frees_their_properties() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.state.players[0].cash = 10;
        game.state.owners[1] = Some(0);

        game.charge(0, 50, Some(1));

        assert!(game.state.players[0].bankrupt);
        assert_eq!(game.state.players[0].cash, 0);
        assert_eq!(
            game.state.owners[1], None,
            "bankrupt player's properties return to the unowned pool"
        );
    }

    #[test]
    fn the_game_ends_once_only_one_player_remains() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.state.players[1].bankrupt = true;

        let result = game.run_to_completion();

        assert_eq!(result.winner, Some(0));
    }

    #[test]
    fn go_salary_is_paid_exactly_once_on_passing_or_landing_on_go() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        let starting_cash = game.state.players[0].cash;

        game.state.players[0].position = 38;
        game.move_player(0, 4); // 38 -> 2, passes GO
        assert_eq!(game.state.players[0].position, 2);
        assert_eq!(
            game.state.players[0].cash,
            starting_cash + game.rules.go_salary as i64
        );

        let after_pass = game.state.players[0].cash;
        game.state.players[0].position = 35;
        game.move_player(0, 5); // 35 -> 0, lands exactly on GO
        assert_eq!(game.state.players[0].position, 0);
        assert_eq!(
            game.state.players[0].cash,
            after_pass + game.rules.go_salary as i64
        );

        let after_land = game.state.players[0].cash;
        game.move_player(0, 3); // 0 -> 3, no wrap
        assert_eq!(
            game.state.players[0].cash, after_land,
            "no salary without passing/landing on GO"
        );
    }
}
