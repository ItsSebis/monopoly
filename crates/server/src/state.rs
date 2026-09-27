use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::interactive::session::Sessions;

/// A single shared connection behind a mutex, not a pool — this is a local,
/// single-user archive with no concurrent-writer load to plan around (see
/// `docs/roadmap.md` Phase 4's scoping notes). Every use is wrapped in
/// `tokio::task::spawn_blocking` since `rusqlite` is synchronous.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    /// Live interactive sessions (Phase 9), keyed by session id. Each entry's
    /// game runs on its own dedicated OS thread (see
    /// `crates/server/src/interactive/session.rs`) — this map (and the
    /// `Arc<SessionHandle>` it holds) is the only thing shared between that
    /// thread and the async handlers. Locking is brief (map lookups, a
    /// `SharedSessionState` clone) so, unlike `db`, this is accessed directly
    /// from async handlers rather than via `spawn_blocking`.
    pub sessions: Sessions,
}
