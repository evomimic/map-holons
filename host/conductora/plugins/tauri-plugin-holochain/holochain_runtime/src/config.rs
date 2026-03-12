use holochain_conductor_api::conductor::NetworkConfig;
use std::path::PathBuf;

pub struct HolochainRuntimeConfig {
    /// The directory where the holochain files and databases will be stored in
    pub holochain_dir: PathBuf,

    // Holochain network config
    pub network_config: NetworkConfig,

    /// Fallback to LAN only mode if the signal server configured in NetworkConfig can't be
    /// reached at launch
    pub fallback_to_lan_only: bool,

    /// Force the conductor to run at this admin port
    pub admin_port: Option<u16>,

    /// Dev mode: skip lair keystore and use an ephemeral in-memory keystore.
    /// Keys are NOT persisted across restarts. Suitable only for local CRUD tests.
    pub dev_mode: bool,
}

impl HolochainRuntimeConfig {
    pub fn new(holochain_dir: PathBuf, network_config: NetworkConfig) -> Self {
        Self {
            holochain_dir,
            network_config,
            admin_port: None,
            fallback_to_lan_only: true,
            dev_mode: false,
        }
    }

    pub fn admin_port(mut self, admin_port: u16) -> Self {
        self.admin_port = Some(admin_port);
        self
    }

    /// Enable dev mode (ephemeral DangerTestKeystore, no lair, ~instant startup).
    pub fn dev_mode(mut self) -> Self {
        self.dev_mode = true;
        self
    }
}
