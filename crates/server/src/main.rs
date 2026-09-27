use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clap::Parser;
use monopoly_server::interactive::session::{Sessions, SESSION_IDLE_TIMEOUT};
use monopoly_server::{build_router, db, AppState};

#[derive(Parser)]
#[command(name = "monopoly-server", about = "Monopoly run archive server")]
struct Cli {
    /// Port to listen on.
    #[arg(long, default_value_t = 3000)]
    port: u16,
    /// Path to the SQLite database file (created if missing).
    #[arg(long, default_value = "./monopoly.db")]
    db: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let conn = db::open(&cli.db).unwrap_or_else(|e| {
        eprintln!("error: opening database {}: {e}", cli.db);
        std::process::exit(1);
    });
    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        sessions: Arc::new(Mutex::new(HashMap::new())),
    };
    spawn_session_reaper(state.sessions.clone());

    let addr = format!("0.0.0.0:{}", cli.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("error: binding {addr}: {e}");
            std::process::exit(1);
        });
    println!("monopoly-server listening on {addr} (db: {})", cli.db);
    axum::serve(listener, build_router(state))
        .await
        .expect("server error");
}

/// Periodically drops interactive sessions (Phase 9) that have gone idle
/// past `SESSION_IDLE_TIMEOUT` — deliberately spawned here rather than inside
/// `build_router`, simply to keep `build_router` itself free of background
/// tasks (a router built for a test shouldn't come with one attached).
fn spawn_session_reaper(sessions: Sessions) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let now = std::time::Instant::now();
            let mut sessions = sessions.lock().expect("sessions mutex poisoned");
            sessions.retain(|_, handle| {
                let last_activity = handle
                    .shared
                    .lock()
                    .expect("session mutex poisoned")
                    .last_activity;
                now.duration_since(last_activity) < SESSION_IDLE_TIMEOUT
            });
        }
    });
}
