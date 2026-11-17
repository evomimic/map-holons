// Module declarations
mod config;
mod constitution_admission;
pub mod env;
mod map_commands;
pub mod personal_constitution;
mod runtime;
mod setup;
pub mod source_ingress;
mod trusted_welcome;

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
