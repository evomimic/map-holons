#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod config;
mod logging;
mod map_commands;
pub mod setup;

fn main() {
    // Initialise logging before anything else.
    // RUST_LOG controls the host-side filter (see src/logging.rs for the full reference).
    // WASM_LOG controls zome log forwarding (holochain reads it; logging.rs also applies it).
    // Examples:
    //   RUST_LOG=host:debug                          host crates at DEBUG, holochain quiet
    //   RUST_LOG=host:debug WASM_LOG=debug           host DEBUG + all zome log events
    //   RUST_LOG=all:debug                           everything at DEBUG (very verbose)
    //   (unset)                                      defaults to host:warn
    logging::init_logging();

    tracing::info!("[MAIN] Starting Conductora runtime");
    conductora_lib::run();
}
