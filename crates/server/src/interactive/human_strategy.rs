//! `HumanStrategy`: a `Strategy` implementation that never decides anything
//! itself. Each of its 7 methods publishes a `PendingDecision` describing
//! what it's being asked, then blocks the current thread on
//! `mpsc::Receiver::recv()` until `POST /sessions/:id/decisions` answers it.
//!
//! This only works because a session's `Game` runs on its own dedicated OS
//! thread (`session::spawn_session`) rather than as an async task: blocking
//! a plain thread costs nothing else on the system, and the real Rust call
//! stack through `Game::step_turn()`'s nested calls IS the resume point —
//! there's no hand-maintained state machine anywhere in here.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use monopoly_engine::state::GameView;
use monopoly_engine::{
    BuildAction, JailAction, MortgageAction, PurchaseOffer, Strategy, TradeOffer,
};

use crate::interactive::decision::{DecisionAnswer, PendingDecision};
use crate::interactive::session::{flush_new_events, SharedSessionState};

#[derive(Debug)]
pub struct HumanStrategy {
    // Not read anywhere yet — every decision hook already carries `player`
    // explicitly (see `Strategy`'s doc comment on why), which for this
    // seat's own hooks always equals `seat`. Kept on the struct since a
    // future multi-human session would need it to tell seats apart.
    #[allow(dead_code)]
    seat: usize,
    shared: Arc<Mutex<SharedSessionState>>,
    answers: mpsc::Receiver<DecisionAnswer>,
}

impl HumanStrategy {
    pub fn new(
        seat: usize,
        shared: Arc<Mutex<SharedSessionState>>,
        answers: mpsc::Receiver<DecisionAnswer>,
    ) -> Self {
        HumanStrategy {
            seat,
            shared,
            answers,
        }
    }

    /// Publishes `decision`, flushing this turn's events so far
    /// (`view.log_since_turn_start`) and refreshing the shared state
    /// snapshot from `view` (mutations up to this point in the turn have
    /// already happened) in the same locked section, then blocks until an
    /// answer arrives. Returns `None` if the channel closed instead — the
    /// session was torn down (`DELETE /sessions/:id` drops the `Sender`)
    /// while this seat was waiting.
    fn ask(&mut self, view: &GameView, decision: PendingDecision) -> Option<DecisionAnswer> {
        {
            let mut shared = self.shared.lock().unwrap();
            flush_new_events(&mut shared, view.log_since_turn_start);
            shared.pending = Some(decision);
            shared.state = view.state.clone();
            shared.last_activity = Instant::now();
        }
        self.answers.recv().ok()
    }
}

impl Strategy for HumanStrategy {
    fn decide_purchase(&mut self, view: &GameView, player: usize, offer: &PurchaseOffer) -> bool {
        let decision = PendingDecision::Purchase {
            player,
            offer: *offer,
        };
        match self.ask(view, decision) {
            Some(DecisionAnswer::Purchase { buy }) => buy,
            _ => false, // safe default: same as every other strategy declining a purchase
        }
    }

    fn decide_jail_action(&mut self, view: &GameView, player: usize) -> JailAction {
        let decision = PendingDecision::JailAction { player };
        match self.ask(view, decision) {
            Some(DecisionAnswer::JailAction { action }) => action,
            // Safe default: keep trying for doubles rather than spend cash
            // the player never chose to spend — the same "no-op" bias every
            // other invalid/missing action here defaults to.
            _ => JailAction::RollForDoubles,
        }
    }

    fn decide_build(&mut self, view: &GameView, player: usize) -> Vec<BuildAction> {
        let decision = PendingDecision::Build { player };
        match self.ask(view, decision) {
            Some(DecisionAnswer::Build { actions }) => actions,
            _ => Vec::new(),
        }
    }

    fn decide_mortgage(
        &mut self,
        view: &GameView,
        player: usize,
        shortfall: u32,
    ) -> Vec<MortgageAction> {
        let decision = PendingDecision::Mortgage { player, shortfall };
        match self.ask(view, decision) {
            Some(DecisionAnswer::Mortgage { actions }) => actions,
            _ => Vec::new(),
        }
    }

    fn decide_auction_bid(&mut self, view: &GameView, player: usize, space: usize) -> Option<u32> {
        let decision = PendingDecision::AuctionBid { player, space };
        match self.ask(view, decision) {
            Some(DecisionAnswer::AuctionBid { amount }) => amount,
            _ => None, // abstain
        }
    }

    fn decide_trade(&mut self, view: &GameView, player: usize) -> Option<TradeOffer> {
        let decision = PendingDecision::TradeProposal { player };
        match self.ask(view, decision) {
            Some(DecisionAnswer::TradeProposal { offer }) => offer,
            _ => None, // propose nothing this turn
        }
    }

    fn decide_trade_response(
        &mut self,
        view: &GameView,
        player: usize,
        offer: &TradeOffer,
    ) -> bool {
        let decision = PendingDecision::TradeResponse {
            player,
            offer: offer.clone(),
        };
        match self.ask(view, decision) {
            Some(DecisionAnswer::TradeResponse { accept }) => accept,
            _ => false, // safe default: decline
        }
    }
}
