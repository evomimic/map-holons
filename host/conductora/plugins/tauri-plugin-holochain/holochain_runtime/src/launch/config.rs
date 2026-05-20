use holochain::conductor::{
    config::{AdminInterfaceConfig, ConductorConfig, KeystoreConfig},
    interface::InterfaceDriver,
};
//use holochain_conductor_api::conductor::DpkiConfig;
use holochain_keystore::paths::KeystorePath;
use holochain_types::websocket::AllowedOrigins;
use url2::Url2;

use crate::{filesystem::FileSystem, NetworkConfig};
//launch::DEVICE_SEED_LAIR_KEYSTORE_TAG,

pub fn conductor_config(
    fs: &FileSystem,
    admin_port: u16,
    lair_root: KeystorePath,
    mut network_config: NetworkConfig,
    //local_signal_url: Option<Url2>,
    dev_mode: bool,
    dev_data_root: Option<std::path::PathBuf>,
) -> ConductorConfig {
    let mut config = ConductorConfig::default();

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
    } else {
        config.data_root_path = Some(fs.conductor_dir().into());
        config.keystore = KeystoreConfig::LairServerInProc { lair_root: Some(lair_root) };
        // config.device_seed_lair_tag = Some(DEVICE_SEED_LAIR_KEYSTORE_TAG.into());
    }

    // config.dpki = DpkiConfig::disabled();

    if dev_mode {
        network_config.bootstrap_url = Url2::parse("http://127.0.0.1:1");
        network_config.relay_url = Url2::parse("https://127.0.0.1:1");
        network_config.target_arc_factor = 0;
        network_config.advanced = None;
    } else {
        if network_config.advanced.is_none() {
            let advanced_config = serde_json::json!({
                "tx5Transport": {
                    "signalAllowPlainText": true,
                },
               "irohTransport": {
                    "relayAllowPlainText": true,
                    "coreBootstrap": {
                        "backoffMaxMs": 20000,
                    },
                },
                "coreSpace": {
                    "reSignExpireTimeMs": 20000,
                    "reSignFreqMs": 20000,
                },
            });
            network_config.advanced = Some(advanced_config);
        }
    }
    config.network = network_config;

    let allowed_origins = AllowedOrigins::Any;

    config.admin_interfaces = Some(vec![AdminInterfaceConfig {
        driver: InterfaceDriver::Websocket {
            port: admin_port,
            danger_bind_addr: None,
            allowed_origins,
        },
    }]);

    config
}
