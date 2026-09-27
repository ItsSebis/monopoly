//! Owns a live interactive game: one dedicated OS thread per session,
//! looping `Game::step_turn()` until the game ends, with a `HumanStrategy`
//! at one seat blocking that same thread until the browser answers.
//!
//! `SessionHandle` (held by `AppState.sessions`) and the session thread
//! share nothing but `SharedSessionState` behind a `Mutex` and the
//! `mpsc` channel `HumanStrategy` blocks on — no cancellation flag is needed
//! to tear a session down: dropping the last `Arc<SessionHandle>` (`DELETE
//! /sessions/:id`) drops the channel's `Sender`, which wakes a blocked
//! `HumanStrategy::recv()` with an `Err`, which resolves to that hook's safe
//! default (see `human_strategy.rs`) and lets the thread run to completion
//! (or the next decision, and so on) and exit on its own.

use std::panic;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use monopoly_engine::state::GameState;
use monopoly_engine::strategies::make_strategy;
use monopoly_engine::{
    ConfigError, Event, EventEnvelope, Game, GameResult, PlayerConfig, RuleSet, Strategy,
    SAFETY_MAX_TURNS,
};

use crate::interactive::decision::{DecisionAnswer, PendingDecision};
use crate::interactive::human_strategy::HumanStrategy;

/// A session with no activity (poll, or decision answered) for this long is
/// dropped by the reaper task in `main.rs`.
pub const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// State shared between a session's dedicated game thread and whichever
/// async handler is currently reading/writing it. Cloned wholesale (into a
/// `SessionSnapshot`, `routes.rs`) on every `GET`/`POST` response rather than
/// handed out by reference, so the lock is only ever held for the duration
/// of a plain data copy.
#[derive(Debug)]
pub struct SharedSessionState {
    pub state: GameState,
    pub pending: Option<PendingDecision>,
    /// Every event produced so far, oldest first — `EventEnvelope::seq` is
    /// already a monotonically increasing id assigned by the engine itself,
    /// which is what `GET /sessions/:id?since_seq=` filters against, so
    /// there's no need for a second, separately-maintained counter here.
    pub events: Vec<EventEnvelope>,
    /// The most recent event's own `seq` (0 before any event has fired).
    pub seq: u64,
    pub game_over: Option<GameResult>,
    /// Set instead of `game_over` if the session thread panicked — a caught
    /// panic (`spawn_session`'s `catch_unwind`) publishes this terminal state
    /// rather than leaving the session silently hung with no thread left to
    /// answer it.
    pub errored: Option<String>,
    pub last_activity: Instant,
}

impl SharedSessionState {
    fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Whether the session can still accept a decision answer.
    pub fn is_finished(&self) -> bool {
        self.game_over.is_some() || self.errored.is_some()
    }
}

pub enum SubmitError {
    NothingPending,
    KindMismatch,
    Finished,
}

pub struct SessionHandle {
    pub id: String,
    pub human_seat: usize,
    pub shared: Arc<Mutex<SharedSessionState>>,
    // A plain `mpsc::Sender` rather than bare — wrapped in a `Mutex` so
    // `SessionHandle` (shared via `Arc` across concurrent requests) doesn't
    // need to lean on `Sender`'s own (version-dependent) `Sync` bound; sends
    // are rare (one per answered decision) so the extra lock is free.
    answers_tx: Mutex<mpsc::Sender<DecisionAnswer>>,
}

impl SessionHandle {
    pub fn touch(&self) {
        self.shared.lock().unwrap().touch();
    }

    /// Validates `answer` against the currently-pending decision and, if it
    /// matches, clears `pending` and forwards it to the blocked
    /// `HumanStrategy` — in that order, under the same lock, so a `GET`
    /// racing this call never observes a decision that's already been
    /// answered but not yet cleared.
    pub fn submit_answer(&self, answer: DecisionAnswer) -> Result<(), SubmitError> {
        let mut shared = self.shared.lock().unwrap();
        if shared.is_finished() {
            return Err(SubmitError::Finished);
        }
        match &shared.pending {
            None => return Err(SubmitError::NothingPending),
            Some(pending) if pending.kind() != answer.kind() => {
                return Err(SubmitError::KindMismatch)
            }
            Some(_) => {}
        }
        shared.pending = None;
        shared.touch();
        drop(shared);

        // The receiving `HumanStrategy` may already be gone (session thread
        // exited between the checks above and this send, e.g. it just hit
        // the turn cap) — a `send` error just means the answer arrived too
        // late to matter, not a client-facing failure.
        let _ = self.answers_tx.lock().unwrap().send(answer);
        Ok(())
    }
}

/// Builds the seat vector (CPU seats via `make_strategy`, the human seat via
/// `HumanStrategy`), constructs the `Game`, and spawns its dedicated thread.
/// Returns the same `ConfigError`s `Game::new` would for a bad player list —
/// `human_seat` itself is validated by the caller (the route handler), since
/// "which seat is human" isn't a concept `ConfigError` has a variant for.
pub fn spawn_session(
    id: String,
    rules: RuleSet,
    players: Vec<PlayerConfig>,
    human_seat: usize,
    seed: u64,
) -> Result<Arc<SessionHandle>, ConfigError> {
    if players.len() < 2 {
        return Err(ConfigError::NotEnoughPlayers(players.len()));
    }
    let names: Vec<String> = players.iter().map(|p| p.name.clone()).collect();

    // The initial (turn-0) snapshot, published immediately so the handler
    // that spawns this session has something real to respond with — built
    // directly rather than by throwing away a placeholder `Game`, since
    // `GameState::new` needs nothing a `Game` does beyond `rules`/`names`.
    let initial_state = GameState::new(&rules, &names);
    let shared = Arc::new(Mutex::new(SharedSessionState {
        state: initial_state,
        pending: None,
        events: Vec::new(),
        seq: 0,
        game_over: None,
        errored: None,
        last_activity: Instant::now(),
    }));

    let (tx, rx) = mpsc::channel::<DecisionAnswer>();
    let mut rx = Some(rx); // there's only one human seat, so this is taken exactly once
    let mut strategies: Vec<Box<dyn Strategy + Send>> = Vec::with_capacity(players.len());
    for (seat, player) in players.iter().enumerate() {
        if seat == human_seat {
            let rx = rx.take().expect("only one seat is ever the human seat");
            strategies.push(Box::new(HumanStrategy::new(seat, shared.clone(), rx)));
        } else {
            let strategy = make_strategy(&player.strategy)
                .ok_or_else(|| ConfigError::UnknownStrategy(player.strategy.clone()))?;
            strategies.push(strategy);
        }
    }
    // Read before `with_strategies` moves `rules` — the session thread's own
    // loop condition, mirroring `Game::run_to_completion`'s (see
    // `SAFETY_MAX_TURNS`'s doc comment on why `step_turn` itself enforces no
    // cap of its own).
    let turn_cap = rules.max_turns.unwrap_or(SAFETY_MAX_TURNS);
    let game = Game::with_strategies(rules, &names, strategies, seed)?;

    let handle = Arc::new(SessionHandle {
        id,
        human_seat,
        shared: shared.clone(),
        answers_tx: Mutex::new(tx),
    });

    thread::spawn(move || run_session(game, turn_cap, shared));

    Ok(handle)
}

/// The session thread's body: step the game to completion (or the turn
/// cap), publishing state/events after every turn, then record the terminal
/// outcome. Wrapped in `catch_unwind` by the caller... actually done here so
/// this function alone owns the whole thread's lifetime.
fn run_session(game: Game, turn_cap: u32, shared: Arc<Mutex<SharedSessionState>>) {
    let outcome = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        step_until_done(game, turn_cap, &shared)
    }));
    if let Err(payload) = outcome {
        let message = panic_message(&payload);
        let mut locked = shared.lock().unwrap();
        // A mid-turn panic can leave a decision `pending` with nobody left
        // to answer it (the thread that would have read the answer is
        // gone) — clear it so a client doesn't poll forever waiting for a
        // decision that can never resolve.
        locked.pending = None;
        locked.errored = Some(message);
        locked.touch();
    }
}

fn step_until_done(mut game: Game, turn_cap: u32, shared: &Arc<Mutex<SharedSessionState>>) {
    while !game.is_over() && game.state().turn < turn_cap {
        let new_events = game.step_turn().to_vec();
        let mut locked = shared.lock().unwrap();
        if let Some(last) = new_events.last() {
            locked.seq = last.seq;
        }
        locked.events.extend(new_events);
        locked.state = game.state().clone();
        locked.touch();
    }
    finalize(&game, shared);
}

/// Determines the winner (if any) the same way `Game::run_to_completion`
/// does, and publishes it as both a synthetic `GameEnded` event (so a client
/// that only ever reads `events` still sees one, matching every other
/// driver of the engine) and `SharedSessionState.game_over`. Built entirely
/// from `Game`'s public accessors — `Game::run_to_completion` isn't reused
/// here since it would `mem::take` the *entire* internal log (everything
/// since turn 1), which `step_until_done` has already been publishing
/// incrementally; calling it would republish every prior event a second
/// time.
fn finalize(game: &Game, shared: &Arc<Mutex<SharedSessionState>>) {
    let winner = match game.state().active_player_count() {
        1 => game.state().players.iter().position(|p| !p.bankrupt),
        _ => None,
    };
    let turns = game.state().turn;
    let closing_player = winner.unwrap_or(game.state().current_player);

    let mut locked = shared.lock().unwrap();
    locked.seq += 1;
    let seq = locked.seq;
    locked.events.push(EventEnvelope {
        turn: turns,
        player: closing_player,
        seq,
        event: Event::GameEnded { winner, turns },
    });
    locked.state = game.state().clone();
    // `events` is left empty here rather than duplicating
    // `SharedSessionState.events` a second time inside every future
    // snapshot response — the full log is already available incrementally
    // via `events`/`since_seq`.
    locked.game_over = Some(GameResult {
        winner,
        turns,
        events: Vec::new(),
        final_state: game.state().clone(),
    });
    locked.touch();
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "the game thread panicked".to_string()
    }
}
