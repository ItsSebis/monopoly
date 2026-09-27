//! Phase 9: server-authoritative interactive human-vs-CPU play. See
//! `session.rs` for the "one dedicated OS thread per session, with a
//! blocking `HumanStrategy`" design this module is built around.

pub mod board;
pub mod decision;
pub mod human_strategy;
pub mod routes;
pub mod session;
