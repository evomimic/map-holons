// Module declarations
mod commands;
mod config;
mod setup;
//mod utils;

// Re-exports for clean API
//pub use config::APP_ID;
pub use setup::AppBuilder;

/// Main entry point for the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing::info!("[MAIN APP] Starting application.");

    AppBuilder::build()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}