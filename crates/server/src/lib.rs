//! The Phase 4 archive server (`docs/api.md`, `docs/roadmap.md` Phase 4):
//! axum + SQLite, durably archiving single/batch runs and regenerating any
//! one game from a batch on demand by re-running the engine natively.

pub mod db;
pub mod error;
pub mod handlers;
pub mod ids;
pub mod state;

pub use handlers::build_router;
pub use state::AppState;
