use crate::config::providers::ProviderConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub data_dir: PathBuf,
    pub max_size_mb: Option<u64>,
    pub compression: bool,
    pub encryption: bool,
    #[serde(default)]
    pub features: Vec<String>,
    pub enabled: bool,
}

//todo: add common functions
impl ProviderConfig for LocalConfig {
}
