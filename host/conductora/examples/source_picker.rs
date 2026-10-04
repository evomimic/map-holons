//! Native smoke harness: cargo run --manifest-path host/Cargo.toml -p Conductora
//! --example source_picker -- mixed|files|directories
use conductora_lib::source_ingress::{
    discover_sources, pick_sources, SourceSelection, SourceSelectionMode,
};

fn main() -> Result<(), String> {
    let mode = match std::env::args().nth(1).as_deref() {
        Some("files") => SourceSelectionMode::Files,
        Some("directories") => SourceSelectionMode::Directories,
        Some("mixed") => SourceSelectionMode::Mixed,
        _ => return Err("Specify mixed, files, or directories".into()),
    };
    let result = match pick_sources(mode)? {
        Some(paths) => SourceSelection::Selected(discover_sources(paths)),
        None => SourceSelection::Cancelled,
    };
    println!("{}", serde_json::to_string_pretty(&result).map_err(|error| error.to_string())?);
    Ok(())
}
