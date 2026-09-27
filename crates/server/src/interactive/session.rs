//! Owns a live interactive game: one dedicated OS thread per session,
//! looping `Game::step_turn()` until the game ends, with a `HumanStrategy`
//! at one seat blocking that same thread until the browser answers.
//!
//! `SessionHandle` (held by `AppState.sessions`) and the session thread
//! share nothing but `SharedSessionState` behind a `Mutex` and the `mpsc`
//! channel `HumanStrategy` blocks its `Receiver::recv()` on. Tearing a
//! session down needs no cancellation flag: `DELETE /sessions/:id` drops the
//! session's `Arc<SessionHandle>`, which drops its `Sender`, which wakes a
//! blocked `recv()` with an `Err` — `HumanStrategy` resolves that to the
//! same safe default it uses for any other closed-channel case (see
//! `human_strategy.rs`), and the thread simply runs on to completion (or its
//! next decision) and exits normally. A panic is unrelated and handled
//! separately: `catch_unwind` in `run_session` catches it and publishes
//! `SharedSessionState.errored` instead, since there's no thread left in
//! that case to ever answer a pending decision.

use std::collections::HashMap;
use std::panic;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use monopoly_engine::state::GameState;
use monopoly_engine::strategies::make_strategy;
use monopoly_engine::{ConfigError, EventEnvelope, Game, PlayerConfig, RuleSet, Strategy};
use serde::Serialize;

use crate::interactive::decision::{DecisionAnswer, PendingDecision};
use crate::interactive::human_strategy::HumanStrategy;

/// A session with no activity (poll, or decision answered) for this long is
/// dropped by the reaper task in `main.rs`.
pub const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// The live-session map `AppState.sessions` holds and the reaper task in
/// `main.rs` sweeps — named so the type isn't spelled out twice.
pub type Sessions = Arc<Mutex<HashMap<String, Arc<SessionHandle>>>>;

/// A session's terminal outcome — deliberately not `GameResult` (which also
/// carries the full event log and a final `GameState`, both already
/// available from `SharedSessionState.events`/`state` and would otherwise be
/// duplicated in every snapshot response from here on).
#[derive(Debug, Clone, Serialize)]
pub struct GameOver {
    pub winner: Option<usize>,
    pub turns: u32,
}

/// State shared between a session's dedicated game thread and whichever
/// async handler is currently reading/writing it. Cloned wholesale (into a
/// `SessionSnapshot`, `routes.rs`) on every `GET`/`POST` response rather than
/// handed out by reference, so the lock is only ever held for the duration
/// of a plain data copy.
#[derive(Debug)]
pub struct SharedSessionState {
    pub state: GameState,
    pub pending: Option<PendingDecision>,
    /// Every event produced so far, oldest first.
    pub events: Vec<EventEnvelope>,
    /// The most recently published event's own `seq` (0 before any event has
    /// fired) — `EventEnvelope::seq` is already a monotonically increasing
    /// id assigned by the engine itself, so this doubles as both the
    /// `since_seq` watermark `GET`/`POST` responses filter against and the
    /// "already flushed up to here" marker `flush_new_events` uses to avoid
    /// publishing the same event twice (once mid-turn via `HumanStrategy`,
    /// once more at turn end).
    pub seq: u64,
    pub game_over: Option<GameOver>,
    /// Set instead of `game_over` if the session thread panicked — a caught
    /// panic (`run_session`'s `catch_unwind`) publishes this terminal state
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

/// Appends every event in `events` past what's already been published
/// (`shared.seq`), advancing `shared.seq` to the last one appended.
/// `events` must be seq-ordered (true of any slice the engine hands back).
/// Used identically whether flushing a turn's events so far as a decision is
/// published mid-turn (`HumanStrategy::ask`) or its remaining events once the
/// turn ends (`step_until_done`) — the shared `seq` watermark is what keeps
/// an event flushed by the first from being republished by the second.
pub(crate) fn flush_new_events(shared: &mut SharedSessionState, events: &[EventEnvelope]) {
    let already_published = events.partition_point(|e| e.seq <= shared.seq);
    let new = &events[already_published..];
    if let Some(last) = new.last() {
        shared.seq = last.seq;
    }
    shared.events.extend_from_slice(new);
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
    answers_tx: mpsc::Sender<DecisionAnswer>,
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
            Some(pending) if !answer.answers(pending) => return Err(SubmitError::KindMismatch),
            Some(_) => {}
        }
        shared.pending = None;
        shared.touch();
        drop(shared);

        // The receiving `HumanStrategy` may already be gone (session thread
        // exited between the checks above and this send, e.g. it just hit
        // the turn cap) — a `send` error just means the answer arrived too
        // late to matter, not a client-facing failure.
        let _ = self.answers_tx.send(answer);
        Ok(())
    }
}

/// Builds the seat vector (CPU seats via `make_strategy`, the human seat via
/// `HumanStrategy`), constructs the `Game`, and spawns its dedicated thread.
/// Returns the same `ConfigError`s `Game::with_strategies` would for a bad
/// player list (fewer than 2, or an unrecognized non-human strategy id) —
/// `human_seat` itself is validated by the caller (the route handler), since
/// "which seat is human" isn't a concept `ConfigError` has a variant for.
pub fn spawn_session(
    id: String,
    rules: RuleSet,
    players: Vec<PlayerConfig>,
    human_seat: usize,
    seed: u64,
) -> Result<Arc<SessionHandle>, ConfigError> {
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
    let game = Game::with_strategies(rules, &names, strategies, seed)?;

    let handle = Arc::new(SessionHandle {
        id,
        human_seat,
        shared: shared.clone(),
        answers_tx: tx,
    });

    thread::spawn(move || run_session(game, shared));

    Ok(handle)
}

/// The session thread's whole body: run the game to completion (or a caught
/// panic), then, only on a panic, publish `errored`. `step_until_done` itself
/// publishes `game_over` in the normal case.
fn run_session(game: Game, shared: Arc<Mutex<SharedSessionState>>) {
    let outcome = panic::catch_unwind(panic::AssertUnwindSafe(|| step_until_done(game, &shared)));
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

/// Steps `game` to completion (or its turn cap), publishing state/events
/// after every turn, then finishes it and publishes the terminal outcome.
fn step_until_done(mut game: Game, shared: &Arc<Mutex<SharedSessionState>>) {
    let turn_cap = game.turn_cap();
    while !game.is_over() && game.state().turn < turn_cap {
        let new_events = game.step_turn().to_vec();
        publish(shared, &new_events, game.state());
    }
    // `finish` may have nothing left to add (`HumanStrategy::ask` can have
    // already flushed everything up to this point mid-turn via the shared
    // `seq` watermark) — `publish` handles an empty/already-seen slice fine.
    let closing_events = game.finish().to_vec();
    publish(shared, &closing_events, game.state());

    let mut locked = shared.lock().unwrap();
    locked.game_over = Some(GameOver {
        winner: game.winner(),
        turns: game.state().turn,
    });
    locked.touch();
}

fn publish(shared: &Arc<Mutex<SharedSessionState>>, events: &[EventEnvelope], state: &GameState) {
    let mut locked = shared.lock().unwrap();
    flush_new_events(&mut locked, events);
    locked.state = state.clone();
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
