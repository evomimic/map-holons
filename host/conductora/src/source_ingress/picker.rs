use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceSelectionMode {
    Mixed,
    Files,
    Directories,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePickerCapabilities {
    pub mixed_selection: bool,
    pub multiple_files: bool,
    pub multiple_directories: bool,
}

/// API capabilities, not a claim that native smoke verification has passed.
#[tauri::command]
pub fn source_picker_capabilities() -> SourcePickerCapabilities {
    SourcePickerCapabilities {
        mixed_selection: cfg!(target_os = "macos"),
        multiple_files: true,
        multiple_directories: true,
    }
}

/// Invoke on the main thread on macOS; Linux portal dialogs may run on a worker.
/// RFD reports a dismissed dialog as None; backend failures may also return None.
pub fn pick_sources(mode: SourceSelectionMode) -> Result<Option<Vec<PathBuf>>, String> {
    let dialog = rfd::FileDialog::new().set_title("Select sources to load");
    // Filtering happens during discovery so mixed-case extensions behave identically
    // across native dialog backends, and directories remain selectable.
    match mode {
        SourceSelectionMode::Files => Ok(dialog.pick_files()),
        SourceSelectionMode::Directories => Ok(dialog.pick_folders()),
        SourceSelectionMode::Mixed => {
            #[cfg(target_os = "macos")]
            {
                Ok(dialog.pick_files_or_folders())
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("Mixed file/directory selection is unavailable on this platform; choose files or directories".into())
            }
        }
    }
}
