use holochain::conductor::{
    config::{AdminInterfaceConfig, ConductorConfig, KeystoreConfig},
    interface::InterfaceDriver,
};
use holochain_conductor_api::conductor::DpkiConfig;
use holochain_keystore::paths::KeystorePath;
use holochain_types::websocket::AllowedOrigins;
use url2::Url2;

use crate::{filesystem::FileSystem, launch::DEVICE_SEED_LAIR_KEYSTORE_TAG, NetworkConfig};

pub fn conductor_config(
    fs: &FileSystem,
    admin_port: u16,
    lair_root: KeystorePath,
    mut network_config: NetworkConfig,
    local_signal_url: Option<Url2>,
    dev_mode: bool,
    dev_data_root: Option<std::path::PathBuf>,
) -> ConductorConfig {
    let mut config = ConductorConfig::default();
    const DEV_SIGNAL_PLACEHOLDER_URL: &str = "ws://127.0.0.1:1";
    const DEV_BOOTSTRAP_PLACEHOLDER_URL: &str = "http://127.0.0.1:1";

    if dev_mode {
        // Use a FIXED persistent dev directory across restarts.
        //
        // Why a hardcoded absolute path (not std::env::temp_dir()):
        //   Inside Nix shells TMPDIR is a session-specific directory like
        //   /tmp/nix-shell.1TXdRd/ that changes on every new shell invocation.
        //   Using temp_dir() would give a different path each run, losing the
        //   WASM compile cache.  /tmp is always available on macOS/Linux.
        let dev_dir = dev_data_root.expect("dev_mode=true requires dev_data_root");
        tracing::info!(
            "[LAUNCH] DEV MODE: using persistent dev conductor dir {:?} (WASM cache preserved)",
            dev_dir
        );
        config.data_root_path = Some(dev_dir.to_path_buf().into());

        // In-memory keystore — no lair process, no argon2 KDF, no device seed.
        config.keystore = KeystoreConfig::DangerTestKeystore;
        config.danger_generate_throwaway_device_seed = true;
    } else {
        config.data_root_path = Some(fs.conductor_dir().into());
        config.keystore = KeystoreConfig::LairServerInProc { lair_root: Some(lair_root) };
        config.device_seed_lair_tag = Some(DEVICE_SEED_LAIR_KEYSTORE_TAG.into());
    }

    config.dpki = DpkiConfig::disabled();

    if dev_mode {
        if let Some(local_signal_url) = local_signal_url {
            tracing::info!(
                "[LAUNCH] DEV MODE: overriding network signal_url with local runtime URL {}",
                local_signal_url.as_str()
            );
            network_config.signal_url = local_signal_url;
        } else {
            let local_only_signal = Url2::parse(DEV_SIGNAL_PLACEHOLDER_URL);
            tracing::debug!(
                "[LAUNCH] DEV MODE: no local signal override provided; forcing local-only signal_url {}",
                local_only_signal.as_str()
            );
            network_config.signal_url = local_only_signal;
        }
        let local_only_bootstrap = Url2::parse(DEV_BOOTSTRAP_PLACEHOLDER_URL);
        tracing::debug!(
            "[LAUNCH] DEV MODE: forcing local-only bootstrap_url {}",
            local_only_bootstrap.as_str()
        );
        network_config.bootstrap_url = local_only_bootstrap;
        network_config.target_arc_factor = 0;
        tracing::debug!(
            "[LAUNCH] DEV MODE: target_arc_factor set to 0 (leech mode, minimal network participation)"
        );

        // Dev-mode networking defaults: fast local behavior.
        let advanced_config = serde_json::json!({
            "tx5Transport": {
                "signalAllowPlainText": true,
            },
            "k2Gossip": {
                "initiateIntervalMs": 500,
                "minInitiateIntervalMs": 0,
            },
        });
        network_config.advanced = Some(advanced_config);
    } else {
        if let Some(local_signal_url) = local_signal_url {
            tracing::debug!(
                "[LAUNCH] Normal mode: overriding network signal_url with local runtime URL {}",
                local_signal_url.as_str()
            );
            network_config.signal_url = local_signal_url;
        } else {
            tracing::debug!(
                "[LAUNCH] Normal mode: no local signal override provided; using configured/default signal_url {}",
                network_config.signal_url.as_str()
            );
        }
        if network_config.advanced.is_none() {
            let advanced_config = serde_json::json!({
                "tx5Transport": {
                    "signalAllowPlainText": true,
                },
                // Gossip faster to speed up the test.
                "k2Gossip": {
                    "initiateIntervalMs": 1000,
                    "minInitiateIntervalMs": 0,
                },
            });
            network_config.advanced = Some(advanced_config);
        }
    }
    config.network = network_config;

    // TODO: uncomment when we can set a custom origin for holochain-client-rust
    // let mut origins: HashSet<String> = HashSet::new();
    // origins.insert(String::from("localhost")); // Compatible with the url of the main window: tauri://localhost
    // let allowed_origins = AllowedOrigins::Origins(origins);

    let allowed_origins = AllowedOrigins::Any;

    config.admin_interfaces = Some(vec![AdminInterfaceConfig {
        driver: InterfaceDriver::Websocket { port: admin_port, allowed_origins },
    }]);

    config
}
