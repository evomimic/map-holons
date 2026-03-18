use crate::config::ProviderConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub data_dir: PathBuf,
    pub max_size_mb: Option<u64>,
    pub compression: bool,
    pub encryption: bool,
    pub snapshot_recovery: Option<bool>,
    pub enabled: bool,
}

impl ProviderConfig for LocalConfig {
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn snapshot_recovery(&self) -> bool {
        self.snapshot_recovery == Some(true)
    }
}
