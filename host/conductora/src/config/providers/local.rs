use crate::config::providers::ProviderConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub data_dir: PathBuf,
    pub max_size_mb: Option<u64>,
    pub compression: bool,
    pub encryption: bool,
    pub enabled: bool,
}

impl ProviderConfig for LocalConfig {
   // fn snapshot_recovery(&self) -> bool {
   //     self.enabled
   // }
}
