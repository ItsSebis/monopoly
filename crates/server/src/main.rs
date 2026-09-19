use std::sync::{Arc, Mutex};

use clap::Parser;
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
    };

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
