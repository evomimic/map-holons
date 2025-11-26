#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing_subscriber::EnvFilter;
use std::str::FromStr;

fn main() {
    // Initialize logger ONCE (like your tests do)
    let env_filter = match std::env::var("RUST_LOG") {
        Ok(val) => {
            eprintln!("[MAIN] Using RUST_LOG from environment: {}", val);
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        }
        Err(_) => {
            eprintln!("[MAIN] RUST_LOG not set, using defaults");
            EnvFilter::from_str(
                "info,\
                holochain_receptor=info,\
                tracing=warn,\
                holochain=warn,\
                holochain_sqlite=error,\
                kitsune2_core=error,\
                kitsune2_gossip=error,\
                kitsune2_dht=error,\
                holochain_types=warn"
            )
                .expect("Failed to parse filter")
        }
    };

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stdout)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .pretty()
        .finish();

    // Set as global default - this prevents Holochain from overriding it
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set subscriber");

    tracing::info!("[MAIN] Starting Conductora runtime");
    
    conductora_lib::run();
}
