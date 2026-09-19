use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;

use crate::board::{
    Board, ColorGroup, SpaceKind, BOARD_SIZE, JAIL_SPACE, RAILROAD_SPACES, UTILITY_SPACES,
};
use crate::building::{can_build, can_sell};
use crate::cards::{standard_decks, CardEffect, Deck, DeckKind};
use crate::config::PlayerConfig;
use crate::events::{Event, EventEnvelope, JailReason, TaxKind};
use crate::rules::RuleSet;
use crate::state::{GameState, GameView, PropertyState};
use crate::strategies::make_strategy;
use crate::strategy::{BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy};

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
    chance: Deck,
    community_chest: Deck,
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
        let mut rng = StdRng::seed_from_u64(seed);
        let (chance, community_chest) = standard_decks(&mut rng);
        Ok(Game {
            board: Board::standard(),
            rules,
            state,
            strategies,
            rng,
            chance,
            community_chest,
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
        // Moved out rather than cloned: the log reaches tens of thousands of
        // envelopes in a long game, and the `Game` is done with it.
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

    fn deck_mut(&mut self, kind: DeckKind) -> &mut Deck {
        match kind {
            DeckKind::Chance => &mut self.chance,
            DeckKind::CommunityChest => &mut self.community_chest,
        }
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
            if !self.state.players[player].bankrupt {
                self.resolve_building_phase(player);
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

    /// Moves `player` forward by `spaces`, paying GO salary if this passes
    /// or lands exactly on GO.
    fn move_player(&mut self, player: usize, spaces: u8) {
        let old = self.state.players[player].position;
        let new = (old + spaces as usize) % BOARD_SIZE;
        self.state.players[player].position = new;
        self.record(player, Event::Move { from: old, to: new });
        // `spaces > 0` keeps a zero-distance `move_toward` (`new == old`)
        // from misreading as "wrapped past GO". No current card can land a
        // player where they already stand, but nothing here relies on that.
        if spaces > 0 && new <= old {
            self.pay_from_bank(player, self.rules.go_salary);
            self.record(player, Event::PassGo);
        }
    }

    /// Moves `player` forward to land exactly on `target` — for "advance to"
    /// card effects, which reuse `move_player`'s normal GO-salary-on-wrap
    /// behavior (every one of these cards' official text confirms this).
    fn move_toward(&mut self, player: usize, target: usize) {
        let current = self.state.players[player].position;
        let forward = (target + BOARD_SIZE - current) % BOARD_SIZE;
        self.move_player(player, forward as u8);
    }

    fn pay_from_bank(&mut self, player: usize, amount: u32) {
        self.state.players[player].cash += amount as i64;
    }

    /// Deducts `amount` from `player`, crediting `payee` if given. If
    /// `player` can't cover it, `raise_cash` is tried first (selling
    /// houses/mortgaging); if that still isn't enough, they go bankrupt —
    /// to `payee` if `Some` (bankruptcy-to-player: `payee` receives what's
    /// left), or to the bank if `None` (properties return to the unowned
    /// pool). A payment with no payee that isn't fully raised is never
    /// completed; a payee always receives exactly `amount` or the payer
    /// goes bankrupt instead of paying partially.
    fn charge(&mut self, player: usize, amount: u32, payee: Option<usize>) {
        if self.state.players[player].cash < amount as i64 {
            let shortfall = (amount as i64 - self.state.players[player].cash).max(0) as u32;
            self.raise_cash(player, shortfall);
        }
        if self.state.players[player].cash >= amount as i64 {
            self.state.players[player].cash -= amount as i64;
            match payee {
                Some(payee) => self.state.players[payee].cash += amount as i64,
                None if self.rules.free_parking_pot => self.state.free_parking_pot += amount,
                None => {} // money simply leaves play (official rules, free_parking_pot disabled)
            }
        } else {
            self.bankrupt_player(player, payee);
        }
    }

    /// Asks `player`'s strategy how to raise `shortfall` in cash (selling
    /// houses and/or mortgaging), and applies whatever it returns. Invalid
    /// actions are silently skipped (see `MortgageAction`'s doc comment).
    fn raise_cash(&mut self, player: usize, shortfall: u32) {
        let (strategy, view) = self.strategy_and_view(player);
        let actions = strategy.decide_mortgage(&view, player, shortfall);
        for action in actions {
            match action {
                MortgageAction::SellHouse(space) => {
                    self.try_sell_house(player, space);
                }
                MortgageAction::Mortgage(space) => {
                    self.try_mortgage(player, space);
                }
            }
        }
    }

    fn bankrupt_player(&mut self, player: usize, payee: Option<usize>) {
        // Whatever the player did manage to raise still changes hands: a
        // creditor receives it along with the properties and cards (official
        // rules); only a bankruptcy to the bank takes it out of play.
        let remaining_cash = self.state.players[player].cash.max(0);
        self.state.players[player].cash = 0;
        self.state.players[player].bankrupt = true;
        self.state.players[player].in_jail = false;
        self.state.players[player].jail_turns = 0;

        // Real bankruptcy resolution always liquidates houses/hotels first
        // (sold back to the bank) before any property changes hands, so a
        // bankrupt transfer never carries buildings — see docs/game-rules.md.
        // A transferred property keeps its mortgage state; one returned to the
        // bank is reset to unowned and unmortgaged.
        for space in 0..BOARD_SIZE {
            if self.state.properties[space].owner != Some(player) {
                continue;
            }
            let houses = self.state.properties[space].houses;
            if houses == 5 {
                self.state.bank_hotels_remaining += 1;
            } else {
                self.state.bank_houses_remaining += houses;
            }
            self.state.properties[space].houses = 0;
            match payee {
                Some(payee) => self.state.properties[space].owner = Some(payee),
                None => self.state.properties[space] = PropertyState::default(),
            }
        }

        let goojf_cards = std::mem::take(&mut self.state.players[player].goojf_cards);
        match payee {
            Some(payee) => {
                self.state.players[payee].cash += remaining_cash;
                self.state.players[payee].goojf_cards.extend(goojf_cards);
            }
            None => {
                for deck_kind in goojf_cards {
                    self.deck_mut(deck_kind).return_get_out_of_jail_free_card();
                }
            }
        }
        self.record(player, Event::Bankrupted { payee });
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

    /// Resolves the jail decision at the start of a jailed player's turn. A
    /// held "Get Out of Jail Free" card is always played automatically first
    /// (see `Strategy`'s doc comment for why this isn't a `Strategy`
    /// decision). Otherwise, official rules give up to 3 turns to roll
    /// doubles; if the third roll also fails, the player must pay the fine
    /// immediately and move using that same roll (see docs/game-rules.md's
    /// Jail section).
    ///
    /// Returns whether the player should go on to take a normal turn this
    /// same call (true after using a card or voluntarily paying the fine —
    /// they still need to roll and move; false in every case that already
    /// moved them this call, since neither a successful escape roll nor the
    /// forced third-attempt move grants the usual "doubles = go again" bonus).
    fn resolve_jail_start_of_turn(&mut self, player: usize) -> bool {
        if let Some(deck_kind) = self.state.players[player].goojf_cards.pop() {
            self.deck_mut(deck_kind).return_get_out_of_jail_free_card();
            self.record(player, Event::UsedGetOutOfJailFreeCard);
            self.exit_jail(player);
            return true;
        }

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
            SpaceKind::FreeParking => {
                if self.rules.free_parking_pot && self.state.free_parking_pot > 0 {
                    let amount = std::mem::take(&mut self.state.free_parking_pot);
                    self.pay_from_bank(player, amount);
                }
            }
            SpaceKind::Chance => {
                let effect = self.chance.draw();
                self.apply_card_effect(player, DeckKind::Chance, effect);
            }
            SpaceKind::CommunityChest => {
                let effect = self.community_chest.draw();
                self.apply_card_effect(player, DeckKind::CommunityChest, effect);
            }
            // GO/Jail have no landing effect beyond GO's salary (already
            // handled in move_player) and Jail's "just visiting".
            SpaceKind::Go | SpaceKind::Jail => {}
        }
    }

    /// Records the debt before settling it: `charge` can emit `Mortgaged` /
    /// `HouseSold` / `Bankrupted` events of its own, and those only read
    /// correctly in the log when the payment that triggered them comes first
    /// (see `Event::RentPaid`'s doc comment).
    fn charge_tax(&mut self, player: usize, amount: u32, kind: TaxKind) {
        self.record(player, Event::TaxPaid { amount, kind });
        self.charge(player, amount, None);
    }

    /// Rent's counterpart to `charge_tax`, recording the debt first for the
    /// same log-ordering reason.
    fn charge_rent(&mut self, player: usize, owner: usize, space: usize, amount: u32) {
        self.record(
            player,
            Event::RentPaid {
                to: owner,
                amount,
                space,
            },
        );
        self.charge(player, amount, Some(owner));
    }

    /// Offers `space` for purchase; if declined (or unaffordable) and
    /// `auction_on_decline` is set, runs an auction for it. Shared by normal
    /// landings and the "advance to nearest railroad/utility" card effects,
    /// which can also land on an unowned property.
    fn offer_purchase(&mut self, player: usize, space: usize) {
        let price = self
            .board
            .space(space)
            .price()
            .expect("ownable space has a price");
        self.record(player, Event::PropertyOffered { space, price });
        let (strategy, view) = self.strategy_and_view(player);
        let wants_to_buy = strategy.decide_purchase(&view, player, &PurchaseOffer { space, price });
        let bought = wants_to_buy && self.state.players[player].cash >= price as i64;
        if bought {
            self.state.players[player].cash -= price as i64;
            self.state.properties[space].owner = Some(player);
        }
        self.record(player, Event::PurchaseDecision { space, bought });
        if !bought && self.rules.auction_on_decline {
            self.run_auction(space);
        }
    }

    /// A single sealed-bid round for `space` (see docs/roadmap.md's Phase 2
    /// auction design note): every non-bankrupt player bids once, with no
    /// visibility into anyone else's bid. The highest bid wins, paying the
    /// second-highest bid (or $1 with only one bidder) — approximating how
    /// a live ascending auction actually settles without simulating rounds.
    fn run_auction(&mut self, space: usize) {
        let mut bids: Vec<(usize, u32)> = Vec::new();
        for p in 0..self.state.players.len() {
            if self.state.players[p].bankrupt {
                continue;
            }
            let (strategy, view) = self.strategy_and_view(p);
            let bid = strategy.decide_auction_bid(&view, p, space).unwrap_or(0);
            self.record(
                p,
                Event::AuctionBid {
                    player: p,
                    amount: (bid > 0).then_some(bid),
                },
            );
            if bid > 0 {
                bids.push((p, bid));
            }
        }
        if bids.is_empty() {
            return; // no bidders: stays unowned
        }
        bids.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0))); // highest first; ties go to the lower player index
        let (winner, winning_bid) = bids[0];
        let price = bids.get(1).map_or(1, |&(_, second)| second).max(1);
        debug_assert!(
            price <= winning_bid,
            "the settled price should never exceed the winning bid"
        );
        if self.state.players[winner].cash >= price as i64 {
            self.state.players[winner].cash -= price as i64;
            self.state.properties[space].owner = Some(winner);
            self.record(
                winner,
                Event::AuctionWon {
                    player: winner,
                    space,
                    amount: price,
                },
            );
        }
    }

    fn resolve_ownable_landing(&mut self, player: usize, space: usize, dice_total: u8) {
        match self.state.properties[space].owner {
            None => self.offer_purchase(player, space),
            // A mortgaged property earns its owner no rent.
            Some(owner) if owner != player && !self.state.properties[space].mortgaged => {
                let amount = self.rent_due(space, owner, dice_total);
                self.charge_rent(player, owner, space, amount);
            }
            Some(_) => {}
        }
    }

    fn rent_due(&self, space: usize, owner: usize, dice_total: u8) -> u32 {
        let view = self.view();
        let houses = self.state.properties[space].houses;
        match self.board.space(space) {
            SpaceKind::Street {
                group,
                base_rent,
                house_rent,
                ..
            } => crate::rent::street_rent(
                base_rent,
                view.owns_full_group(owner, group),
                houses,
                house_rent,
            ),
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

    /// `space`'s position within its own color group — the index that pairs
    /// with `group_house_counts` in `building.rs`'s even-build checks.
    fn group_member_index(&self, group: ColorGroup, space: usize) -> usize {
        self.board
            .group_members(group)
            .position(|s| s == space)
            .expect("space is in its own group")
    }

    /// Attempts to build one house/hotel increment on `space` for `player`;
    /// a no-op if the request is invalid (wrong owner, breaks the even-build
    /// rule, insufficient bank supply, or unaffordable) — see
    /// `BuildAction`'s doc comment.
    fn try_build(&mut self, player: usize, space: usize) -> bool {
        let SpaceKind::Street {
            group, house_cost, ..
        } = self.board.space(space)
        else {
            return false;
        };
        if self.state.properties[space].owner != Some(player)
            || self.state.properties[space].mortgaged
        {
            return false;
        }
        if !can_build(
            &self.view().group_house_counts(group),
            self.group_member_index(group, space),
            self.rules.even_build_rule,
        ) {
            return false;
        }
        let current = self.state.properties[space].houses;
        let hotel_supply_ok = current < 4 || self.state.bank_hotels_remaining > 0;
        let house_supply_ok = current == 4 || self.state.bank_houses_remaining > 0;
        if !hotel_supply_ok
            || !house_supply_ok
            || self.state.players[player].cash < house_cost as i64
        {
            return false;
        }
        self.state.players[player].cash -= house_cost as i64;
        if current == 4 {
            self.state.bank_hotels_remaining -= 1;
            self.state.bank_houses_remaining += 4; // the 4 houses return to the bank's supply
            self.state.properties[space].houses = 5;
        } else {
            self.state.bank_houses_remaining -= 1;
            self.state.properties[space].houses += 1;
        }
        self.record(player, Event::HouseBuilt { space });
        true
    }

    /// The reverse of `try_build`: sells one increment back to the bank at
    /// half its build cost. Converting a hotel back to houses needs the bank
    /// to actually have 4 houses to hand over — a known edge case even in
    /// physical play — so that specific sale is skipped if supply is short.
    fn try_sell_house(&mut self, player: usize, space: usize) -> bool {
        let SpaceKind::Street {
            group, house_cost, ..
        } = self.board.space(space)
        else {
            return false;
        };
        if self.state.properties[space].owner != Some(player) {
            return false;
        }
        if !can_sell(
            &self.view().group_house_counts(group),
            self.group_member_index(group, space),
            self.rules.even_build_rule,
        ) {
            return false;
        }
        let current = self.state.properties[space].houses;
        if current == 5 && self.state.bank_houses_remaining < 4 {
            return false;
        }
        self.state.players[player].cash += (house_cost / 2) as i64;
        if current == 5 {
            self.state.bank_hotels_remaining += 1;
            self.state.bank_houses_remaining -= 4;
            self.state.properties[space].houses = 4;
        } else {
            self.state.bank_houses_remaining += 1;
            self.state.properties[space].houses -= 1;
        }
        self.record(player, Event::HouseSold { space });
        true
    }

    /// Mortgages `space` for half its purchase price. Buildings must already
    /// be sold (see `try_sell_house`) — a built-up property can't be
    /// mortgaged directly.
    fn try_mortgage(&mut self, player: usize, space: usize) -> bool {
        let prop = self.state.properties[space];
        if prop.owner != Some(player) || prop.mortgaged || prop.houses > 0 {
            return false;
        }
        // Official rule (docs/game-rules.md's Mortgaging section): every
        // building in the property's color group must be sold back to the
        // bank first, not just the ones on this property — the even-build
        // rule lets one member sit at 0 houses while its neighbors are built.
        if let SpaceKind::Street { group, .. } = self.board.space(space) {
            let group_is_built_up = self
                .board
                .group_members(group)
                .any(|s| self.state.properties[s].houses > 0);
            if group_is_built_up {
                return false;
            }
        }
        let Some(price) = self.board.space(space).price() else {
            return false;
        };
        self.state.players[player].cash += (price / 2) as i64;
        self.state.properties[space].mortgaged = true;
        self.record(player, Event::Mortgaged { space });
        true
    }

    /// Calls `Strategy::decide_build` once, at the end of `player`'s own
    /// turn, and applies whatever it returns (see docs/game-rules.md's
    /// Building houses and hotels section for why this happens once per
    /// turn rather than being tied to a specific landing).
    fn resolve_building_phase(&mut self, player: usize) {
        let (strategy, view) = self.strategy_and_view(player);
        let actions = strategy.decide_build(&view, player);
        for action in actions {
            match action {
                BuildAction::Build(space) => {
                    self.try_build(player, space);
                }
                BuildAction::SellHouse(space) => {
                    self.try_sell_house(player, space);
                }
            }
        }
    }

    fn apply_card_effect(&mut self, player: usize, deck: DeckKind, effect: CardEffect) {
        self.record(player, Event::CardDrawn { deck, effect });
        match effect {
            CardEffect::AdvanceTo(target) => {
                self.move_toward(player, target);
                self.resolve_landing(player, 0); // none of these targets are utilities, so dice_total is unused
            }
            CardEffect::AdvanceToNearestRailroad => self.advance_to_nearest_railroad(player),
            CardEffect::AdvanceToNearestUtility => self.advance_to_nearest_utility(player),
            CardEffect::CollectFromBank(amount) => self.pay_from_bank(player, amount),
            CardEffect::PayBank(amount) => self.charge(player, amount, None),
            CardEffect::CollectFromEachPlayer(amount) => {
                for other in 0..self.state.players.len() {
                    if other != player && !self.state.players[other].bankrupt {
                        self.charge(other, amount, Some(player));
                    }
                }
            }
            CardEffect::PayEachPlayer(amount) => {
                for other in 0..self.state.players.len() {
                    if self.state.players[player].bankrupt {
                        break;
                    }
                    if other != player && !self.state.players[other].bankrupt {
                        self.charge(player, amount, Some(other));
                    }
                }
            }
            CardEffect::PropertyRepairAssessment {
                per_house,
                per_hotel,
            } => {
                let total: u32 = (0..BOARD_SIZE)
                    .filter(|&s| self.state.properties[s].owner == Some(player))
                    .map(|s| match self.state.properties[s].houses {
                        5 => per_hotel,
                        h => per_house * h as u32,
                    })
                    .sum();
                self.charge_tax(player, total, TaxKind::Repair);
            }
            CardEffect::GoBackThreeSpaces => {
                let current = self.state.players[player].position;
                let new = (current + BOARD_SIZE - 3) % BOARD_SIZE;
                self.state.players[player].position = new;
                self.record(
                    player,
                    Event::Move {
                        from: current,
                        to: new,
                    },
                ); // moving backward never pays GO salary
                self.resolve_landing(player, 0);
            }
            CardEffect::GoToJail => self.send_to_jail(player, JailReason::Card),
            CardEffect::GetOutOfJailFree => self.state.players[player].goojf_cards.push(deck),
        }
    }

    /// Moves `player` to the nearest of `spaces` and offers it for purchase
    /// if it's unowned. Returns `(space, owner)` only when rent is owed to
    /// another player, since both "advance to nearest" cards charge a special
    /// rent the caller works out for itself.
    fn advance_to_nearest(&mut self, player: usize, spaces: &[usize]) -> Option<(usize, usize)> {
        let target = nearest(self.state.players[player].position, spaces);
        self.move_toward(player, target);
        match self.state.properties[target].owner {
            None => {
                self.offer_purchase(player, target);
                None
            }
            Some(owner) if owner != player && !self.state.properties[target].mortgaged => {
                Some((target, owner))
            }
            _ => None,
        }
    }

    fn advance_to_nearest_railroad(&mut self, player: usize) {
        let Some((target, owner)) = self.advance_to_nearest(player, &RAILROAD_SPACES) else {
            return;
        };
        let amount = crate::rent::railroad_rent(self.view().owned_railroad_count(owner)) * 2;
        self.charge_rent(player, owner, target, amount);
    }

    fn advance_to_nearest_utility(&mut self, player: usize) {
        let Some((target, owner)) = self.advance_to_nearest(player, &UTILITY_SPACES) else {
            return;
        };
        let (first, second) = self.roll_dice(player);
        self.charge_rent(player, owner, target, 10 * (first + second) as u32);
    }
}

/// The first space in `spaces` reachable by moving forward from `current`,
/// wrapping around the board — used by the two "advance to nearest X" cards.
fn nearest(current: usize, spaces: &[usize]) -> usize {
    spaces
        .iter()
        .copied()
        .find(|&s| s > current)
        .unwrap_or(spaces[0])
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
            let (doubles_rolled, moves_made) = {
                let events = game.step_turn();
                let doubles = events
                    .iter()
                    .filter(|e| matches!(e.event, Event::RollDice { dice } if dice.0 == dice.1))
                    .count();
                let moves = events
                    .iter()
                    .filter(|e| matches!(e.event, Event::Move { .. }))
                    .count();
                (doubles, moves)
            };
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
    fn charging_more_than_a_player_can_raise_bankrupts_them_to_the_named_payee() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.state.players[0].cash = 10;
        game.state.properties[3].owner = Some(0); // Baltic Avenue, price 60: mortgages for 30, so 40 is all they can raise
        let creditor_before = game.state.players[1].cash;

        game.charge(0, 1_000, Some(1));

        assert!(game.state.players[0].bankrupt);
        assert_eq!(game.state.players[0].cash, 0);
        assert_eq!(
            game.state.properties[3].owner,
            Some(1),
            "bankruptcy-to-player transfers remaining properties to the payee"
        );
        assert_eq!(
            game.state.players[1].cash,
            creditor_before + 40,
            "the creditor receives every dollar the debtor managed to raise"
        );
    }

    #[test]
    fn charging_more_than_a_player_can_raise_with_no_payee_bankrupts_them_to_the_bank() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.state.players[0].cash = 10;
        game.state.properties[3].owner = Some(0);
        let other_before = game.state.players[1].cash;

        game.charge(0, 1_000, None);

        assert!(game.state.players[0].bankrupt);
        assert_eq!(game.state.players[0].cash, 0);
        assert_eq!(
            game.state.properties[3].owner, None,
            "bankruptcy-to-bank returns properties to the unowned pool"
        );
        assert_eq!(
            game.state.players[1].cash, other_before,
            "no other player gains from a bankruptcy to the bank"
        );
    }

    #[test]
    fn a_property_cannot_be_mortgaged_while_its_group_still_has_buildings() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        // The even-build rule allows Mediterranean to sit at 0 houses while
        // Baltic has 1; the official mortgage rule still forbids mortgaging
        // either until the whole group is cleared.
        game.state.properties[1].owner = Some(0);
        game.state.properties[3].owner = Some(0);
        game.state.properties[3].houses = 1;

        assert!(
            !game.try_mortgage(0, 1),
            "the group is still built up, so no member may be mortgaged"
        );
        assert!(!game.state.properties[1].mortgaged);

        game.state.properties[3].houses = 0;
        assert!(game.try_mortgage(0, 1), "clear group, mortgage allowed");
    }

    /// A cash-raising plan the engine can only partly execute is worse than
    /// useless: the strategy counts the skipped actions toward the shortfall
    /// and the player goes bankrupt holding assets it never liquidated.
    #[test]
    fn a_cash_raising_plan_actually_raises_what_it_claims_to() {
        let players = vec![
            PlayerConfig {
                name: "A".into(),
                strategy: "buy_all".into(),
            },
            PlayerConfig {
                name: "B".into(),
                strategy: "buy_all".into(),
            },
        ];
        let mut game = Game::new(RuleSet::default(), &players, 0).unwrap();

        // The whole Light Blue group, built unevenly but legally (2/2/1).
        // Selling these needs the even-build rule respected in the plan, and
        // mortgaging any of them needs the whole group cleared first.
        for (space, houses) in [(6usize, 2u8), (8, 2), (9, 1)] {
            game.state.properties[space].owner = Some(0);
            game.state.properties[space].houses = houses;
        }
        game.state.players[0].cash = 0;

        // Everything those three are worth: 5 houses at $25 each ($125) plus
        // mortgages of $50 + $50 + $60 ($160). Asking for exactly that leaves
        // the plan no room to over-count an action the engine would skip.
        game.raise_cash(0, 285);

        assert_eq!(
            game.state.players[0].cash, 285,
            "the plan must actually realize every dollar it counted"
        );
        assert!(
            (0..BOARD_SIZE).all(|s| game.state.properties[s].houses == 0),
            "every house should have been sold before any mortgage"
        );
    }

    #[test]
    fn mortgaging_to_raise_cash_actually_happens_before_bankruptcy_is_declared() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.state.players[0].cash = 10;
        game.state.properties[3].owner = Some(0); // Baltic Avenue, price 60 -> mortgages for 30

        game.charge(0, 35, Some(1)); // 10 + 30 raised = 40, covers 35 without going bankrupt

        assert!(!game.state.players[0].bankrupt);
        assert!(game.state.properties[3].mortgaged);
        assert_eq!(
            game.state.properties[3].owner,
            Some(0),
            "still owned by the original player, just mortgaged"
        );
        assert_eq!(game.state.players[0].cash, 10 + 30 - 35);
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

    #[test]
    fn building_a_hotel_returns_four_houses_to_the_bank_supply() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        // Give player 0 the whole Brown group with 4 houses each, matching
        // the even-build rule, then build the 5th increment (a hotel) on one.
        for &space in &[1usize, 3] {
            game.state.properties[space].owner = Some(0);
            game.state.properties[space].houses = 4;
        }
        game.state.players[0].cash = 10_000;
        let houses_before = game.state.bank_houses_remaining;

        assert!(game.try_build(0, 1));

        assert_eq!(game.state.properties[1].houses, 5);
        assert_eq!(
            game.state.bank_hotels_remaining,
            crate::state::STARTING_HOTELS - 1
        );
        assert_eq!(
            game.state.bank_houses_remaining,
            houses_before + 4,
            "the 4 houses come back to the bank"
        );
    }

    #[test]
    fn a_mortgaged_property_charges_no_rent() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.state.properties[1].owner = Some(1);
        game.state.properties[1].mortgaged = true;
        game.state.players[0].position = 1;
        let cash_before = game.state.players[0].cash;

        game.resolve_landing(0, 7);

        assert_eq!(
            game.state.players[0].cash, cash_before,
            "no rent charged on a mortgaged property"
        );
    }

    #[test]
    fn get_out_of_jail_free_card_is_used_automatically_and_returned_to_its_deck() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.state.players[0].in_jail = true;
        game.state.players[0].goojf_cards.push(DeckKind::Chance);
        let chance_size_before = game.chance.len();

        let events = game.step_turn();
        assert!(events
            .iter()
            .any(|e| matches!(e.event, Event::UsedGetOutOfJailFreeCard)));
        assert!(!game.state.players[0].in_jail);
        assert!(game.state.players[0].goojf_cards.is_empty());
        assert_eq!(
            game.chance.len(),
            chance_size_before + 1,
            "the card returns to the bottom of its deck"
        );
    }

    #[test]
    fn go_back_three_spaces_never_pays_go_salary_even_when_wrapping_past_it() {
        let players = vec![
            PlayerConfig {
                name: "A".into(),
                strategy: "buy_none".into(),
            },
            PlayerConfig {
                name: "B".into(),
                strategy: "buy_none".into(),
            },
        ];
        let mut game = Game::new(RuleSet::default(), &players, 0).unwrap();
        game.state.players[0].position = 1; // going back 3 wraps to space 38

        game.apply_card_effect(0, DeckKind::Chance, CardEffect::GoBackThreeSpaces);

        assert_eq!(game.state.players[0].position, 38);
        assert!(
            !game.log.iter().any(|e| matches!(e.event, Event::PassGo)),
            "moving backward must never pay GO salary"
        );
    }

    #[test]
    fn advance_to_nearest_railroad_card_charges_double_rent() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.state.players[0].position = 0; // nearest railroad from GO is space 5
        game.state.properties[5].owner = Some(1);
        let cash_before = game.state.players[0].cash;

        game.apply_card_effect(0, DeckKind::Chance, CardEffect::AdvanceToNearestRailroad);

        assert_eq!(game.state.players[0].position, 5);
        assert_eq!(
            cash_before - game.state.players[0].cash,
            50,
            "double the normal $25 single-railroad rent"
        );
    }

    #[test]
    fn advance_to_nearest_utility_card_charges_ten_times_the_dice_roll() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.state.players[0].position = 0; // nearest utility from GO is space 12
        game.state.properties[12].owner = Some(1);

        game.apply_card_effect(0, DeckKind::Chance, CardEffect::AdvanceToNearestUtility);

        let dice_total = game
            .log
            .iter()
            .rev()
            .find_map(|e| match e.event {
                Event::RollDice { dice } => Some(dice.0 + dice.1),
                _ => None,
            })
            .expect("the card rolls dice to determine rent");
        let rent_paid = game
            .log
            .iter()
            .rev()
            .find_map(|e| match e.event {
                Event::RentPaid { amount, .. } => Some(amount),
                _ => None,
            })
            .expect("rent should have been charged");
        assert_eq!(rent_paid, 10 * dice_total as u32);
    }

    #[test]
    fn property_repair_assessment_charges_per_house_and_per_hotel() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.state.properties[1].owner = Some(0);
        game.state.properties[1].houses = 2;
        game.state.properties[6].owner = Some(0);
        game.state.properties[6].houses = 5; // hotel
        let cash_before = game.state.players[0].cash;

        game.apply_card_effect(
            0,
            DeckKind::Chance,
            CardEffect::PropertyRepairAssessment {
                per_house: 25,
                per_hotel: 100,
            },
        );

        assert_eq!(cash_before - game.state.players[0].cash, 2 * 25 + 100);
    }

    /// What the winner of the most recent auction actually paid.
    fn last_auction_price(game: &Game) -> Option<u32> {
        game.log.iter().rev().find_map(|e| match e.event {
            Event::AuctionWon { amount, .. } => Some(amount),
            _ => None,
        })
    }

    /// A test-only strategy with a fixed auction bid, for deterministic
    /// auction-settlement tests — the built-in strategies' bids depend on a
    /// heuristic score, not a value a test can pin exactly.
    #[derive(Debug)]
    struct FixedBidder(u32);

    impl Strategy for FixedBidder {
        fn decide_purchase(
            &mut self,
            _view: &GameView,
            _player: usize,
            _offer: &PurchaseOffer,
        ) -> bool {
            false
        }
        fn decide_jail_action(&mut self, _view: &GameView, _player: usize) -> JailAction {
            JailAction::RollForDoubles
        }
        fn decide_build(&mut self, _view: &GameView, _player: usize) -> Vec<BuildAction> {
            Vec::new()
        }
        fn decide_mortgage(
            &mut self,
            _view: &GameView,
            _player: usize,
            _shortfall: u32,
        ) -> Vec<MortgageAction> {
            Vec::new()
        }
        fn decide_auction_bid(
            &mut self,
            _view: &GameView,
            _player: usize,
            _space: usize,
        ) -> Option<u32> {
            Some(self.0)
        }
    }

    #[test]
    fn auction_with_one_bidder_settles_at_the_one_dollar_minimum() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.strategies[0] = Box::new(FixedBidder(100));
        game.strategies[1] = Box::new(FixedBidder(0));

        game.run_auction(1);

        assert_eq!(game.state.properties[1].owner, Some(0));
        assert_eq!(
            last_auction_price(&game),
            Some(1),
            "a sole bidder pays only the $1 minimum"
        );
    }

    #[test]
    fn auction_with_multiple_bidders_settles_at_the_second_highest_bid() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.strategies[0] = Box::new(FixedBidder(100));
        game.strategies[1] = Box::new(FixedBidder(60));

        game.run_auction(1);

        assert_eq!(
            game.state.properties[1].owner,
            Some(0),
            "the higher bidder wins"
        );
        assert_eq!(
            last_auction_price(&game),
            Some(60),
            "pays the runner-up's bid, not its own"
        );
    }

    #[test]
    fn auction_ties_are_won_by_the_lower_player_index() {
        let mut game = Game::new(RuleSet::default(), &two_players(), 0).unwrap();
        game.strategies[0] = Box::new(FixedBidder(50));
        game.strategies[1] = Box::new(FixedBidder(50));

        game.run_auction(1);

        assert_eq!(game.state.properties[1].owner, Some(0));
    }
}
