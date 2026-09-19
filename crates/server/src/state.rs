use std::sync::{Arc, Mutex};

use rusqlite::Connection;

/// A single shared connection behind a mutex, not a pool — this is a local,
/// single-user archive with no concurrent-writer load to plan around (see
/// `docs/roadmap.md` Phase 4's scoping notes). Every use is wrapped in
/// `tokio::task::spawn_blocking` since `rusqlite` is synchronous.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
}
