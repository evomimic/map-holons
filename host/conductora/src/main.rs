#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing_subscriber::EnvFilter;
use std::str::FromStr;

fn main() {
    // Initialize logger ONCE (like your tests do)
    let env_filter = match std::env::var("RUST_LOG") {
        Ok(val) => {
            eprintln!("[MAIN] Using RUST_LOG from environment: {}", val);
            // Expand shorthand like "host=debug" to full list
            let expanded = expand_log_shorthand(&val);
            eprintln!("[MAIN] Expanded to: {}", expanded);
            EnvFilter::from_str(&expanded)
            .unwrap_or_else(|_| EnvFilter::new("info"))
        }
        Err(_) => {
            eprintln!("[MAIN] RUST_LOG not set, using defaults");
             let default_filter =
                "warn,\
                tracing=warn,\
                holochain=warn,\
                holochain_sqlite=error,\
                kitsune2_core=error,\
                kitsune2_gossip=error,\
                kitsune2_dht=error,\
                holochain_types=warn";
                EnvFilter::from_str(default_filter)
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

fn expand_log_shorthand(input: &str) -> String {
    let mut result = String::new();
    
    for part in input.split(',') {
        if let Some((key, level)) = part.split_once('=') {
            match key.trim() {
                "host" => {
                    // Expand "host=debug" to all custom crates
                    result.push_str(&format!(
                        "conductora_lib={},holons_client={},holons_receptor={},holochain_receptor={}",
                        level, level, level, level
                    ));
                }
                _ => {
                    // Pass through other directives as-is
                    result.push_str(part);
                    result.push(',');
                }
            }
        } else {
            result.push_str(part);
            result.push(',');
        }
    }
    
    // Add holochain baseline if not already specified
    if !result.contains("holochain=") {
        result.push_str("holochain=warn,holochain_sqlite=error,kitsune2_core=error,kitsune2_gossip=error,kitsune2_dht=error,holochain_types=warn");
    }
    
    result
}
