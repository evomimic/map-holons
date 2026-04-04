use crate::config::providers::{
    skipped_selection_warning,
    select_single_key_by,
    MultiEntrySelector,
    ProviderConfig,
};
use serde::{Deserialize, Serialize};
pub type CellDetails = Vec<CellDetail>;
pub const WINDOW_PROVIDER_SELECTOR: &str = "holochain";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellDetail {
    pub role_name: String,
    pub zome_name: String,
    pub zome_function: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolochainConfig {
    pub network_seed: Option<String>,
    pub bootstrap_url: Option<String>,
    pub signal_url: Option<String>,
    pub proxy_url: Option<String>,
    pub target_arc_factor: Option<u32>,
    pub app_id: String,
    pub cell_details: Option<CellDetails>,
    pub happ_path: Option<String>, // Path to .happ file if not embedded
    pub enabled: bool,
    #[serde(default)]
    pub production: bool,
}
//todo: add common functions
impl ProviderConfig for HolochainConfig {}

pub struct HolochainSelector;

impl MultiEntrySelector<HolochainConfig> for HolochainSelector {
    const PROVIDER_TYPE: &'static str = "holochain";

    fn select_keys(entries: &[(&str, &HolochainConfig)]) -> Vec<String> {
        select_single_key_by(entries, |key_a, cfg_a, key_b, cfg_b| {
            cfg_b.production.cmp(&cfg_a.production).then_with(|| key_a.cmp(key_b))
        })
    }

    fn warnings(entries: &[(&str, &HolochainConfig)], selected: &[String]) -> Vec<String> {
        skipped_selection_warning("Holochain", entries, selected)
    }

    fn window_aliases() -> &'static [&'static str] {
        &[WINDOW_PROVIDER_SELECTOR]
    }
}
