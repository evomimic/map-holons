//! Native integration supplied by the application shell, never by IPC callers.
use map_commands_runtime::{application_launch::ApplicationSessionPhase, Runtime};
use std::path::PathBuf;
use tauri::AppHandle;

/// Hooks into the already configured application; Welcome owns its workflow and policy.
pub struct HostServices {
    pub startup: fn(&AppHandle) -> Result<(ApplicationSessionPhase, bool), String>,
    pub runtime: fn(&AppHandle) -> Result<Runtime, String>,
    pub environment_base: fn() -> anyhow::Result<PathBuf>,
}
