pub mod holochain;
pub mod ipfs;
pub mod local;

use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct ProviderRuntimeSelection {
    pub runtime_provider_keys: Vec<String>,
    pub window_provider_key: Option<String>,
    pub warnings: Vec<String>,
}

/// Common interface for all provider config types.
pub trait ProviderConfig: serde::Serialize {
    // Missing field (None) or false -> no snapshot store created.
    //fn snapshot_recovery(&self) -> bool {
    //    false
    // }
}

/// Generic selector interface for provider types that may have
/// multiple enabled config entries but require deterministic selection.
pub trait MultiEntrySelector<C> {
    const PROVIDER_TYPE: &'static str;

    fn select_keys(entries: &[(&str, &C)]) -> Vec<String>;

    fn warnings(_entries: &[(&str, &C)], _selected: &[String]) -> Vec<String> {
        Vec::new()
    }

    fn window_aliases() -> &'static [&'static str] {
        &[]
    }
}

/// Select a single provider key deterministically from a multi-entry provider set.
pub fn select_single_key_by<C, F>(entries: &[(&str, &C)], mut rank: F) -> Vec<String>
where
    F: FnMut(&str, &C, &str, &C) -> Ordering,
{
    let mut entries = entries.to_vec();
    entries.sort_by(|(key_a, cfg_a), (key_b, cfg_b)| rank(key_a, cfg_a, key_b, cfg_b));

    entries.first().map(|(key, _)| (*key).to_string()).into_iter().collect()
}

/// Build a common warning for multi-entry provider selection when some enabled entries are skipped.
pub fn skipped_selection_warning<C>(
    provider_label: &str,
    entries: &[(&str, &C)],
    selected: &[String],
) -> Vec<String> {
    if entries.len() <= 1 {
        return Vec::new();
    }

    let mut skipped: Vec<String> = entries
        .iter()
        .map(|(key, _)| (*key).to_string())
        .filter(|key| !selected.contains(key))
        .collect();

    if skipped.is_empty() {
        return Vec::new();
    }

    skipped.sort();
    vec![format!(
        "Multiple enabled {} providers detected. Active={:?}; skipped={:?}",
        provider_label,
        selected.first(),
        skipped
    )]
}
