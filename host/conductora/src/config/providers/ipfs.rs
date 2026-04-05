use crate::config::providers::ProviderConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpfsConfig {
    pub api_url: String,
    pub gateway_url: String,
    pub repo_path: Option<PathBuf>,
    pub swarm_key: Option<String>,
    pub bootstrap_peers: Vec<String>,
    pub snapshot_recovery: Option<bool>,
    pub enabled: bool,
}

//todo: add common functions
impl ProviderConfig for IpfsConfig {
}
