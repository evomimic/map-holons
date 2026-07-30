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

/// Forces the conductor's WAN endpoints to unroutable local addresses for dev mode.
///
/// Every endpoint must be assigned explicitly: `NetworkConfig::default()` carries real public
/// servers (`dev-test-bootstrap2.holochain.org` for both bootstrap and signal, the iroh canary
/// relay for relay), so any field left unset keeps its public default. That gap is exactly how
/// `signal_url` used to leak the public signal server into dev startup while the other two were
/// overridden.
///
/// TLS schemes (`wss`/`https`) deliberately, not `ws`/`http`: `advanced` is cleared below, so
/// neither `signalAllowPlainText` nor `relayAllowPlainText` is set, and both transports reject a
/// plaintext URL without the matching flag (`kitsune2_transport_iroh` / `_tx5`
/// `validate_config`). With TLS schemes, `advanced = None` needs no companion config —
/// re-confirmed against iroh at holochain 0.6.3.
///
/// Split out from [`conductor_config`] so the invariant is unit-testable without standing up a
/// `FileSystem` and a keystore.
fn apply_dev_mode_network_overrides(network_config: &mut NetworkConfig) {
    network_config.bootstrap_url = Url2::parse("http://127.0.0.1:1");
    network_config.signal_url = Url2::parse("wss://127.0.0.1:1");
    network_config.relay_url = Url2::parse("https://127.0.0.1:1");
    network_config.target_arc_factor = 0;
    network_config.advanced = None;
}

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
        apply_dev_mode_network_overrides(&mut network_config);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The regression this guards: `signal_url` was previously never assigned, so dev mode ran
    /// with the public default while `bootstrap_url` and `relay_url` were local. Asserting over
    /// all three catches the same omission on any of them.
    #[test]
    fn dev_mode_points_every_wan_endpoint_at_localhost() {
        let mut network_config = NetworkConfig::default();

        apply_dev_mode_network_overrides(&mut network_config);

        for (field, url) in [
            ("bootstrap_url", &network_config.bootstrap_url),
            ("signal_url", &network_config.signal_url),
            ("relay_url", &network_config.relay_url),
        ] {
            assert_eq!(url.host_str(), Some("127.0.0.1"), "{field} is not local-only: {url}");
        }
    }

    /// With `advanced = None` there is no `signalAllowPlainText` / `relayAllowPlainText`, and the
    /// transports reject a plaintext URL without it. See [`apply_dev_mode_network_overrides`].
    #[test]
    fn dev_mode_endpoints_use_tls_schemes_so_no_plaintext_flag_is_needed() {
        let mut network_config = NetworkConfig::default();

        apply_dev_mode_network_overrides(&mut network_config);

        assert_eq!(network_config.signal_url.scheme(), "wss");
        assert_eq!(network_config.relay_url.scheme(), "https");
        assert!(network_config.advanced.is_none());
    }

    /// Dev mode must not inherit a public endpoint from an incoming `HolochainConfig` either.
    #[test]
    fn dev_mode_overrides_a_config_that_already_carries_public_endpoints() {
        let mut network_config = NetworkConfig::default();
        network_config.signal_url = Url2::parse("wss://dev-test-bootstrap2.holochain.org");
        network_config.relay_url = Url2::parse("https://use1-1.relay.n0.iroh-canary.iroh.link./");

        apply_dev_mode_network_overrides(&mut network_config);

        let rendered = format!(
            "{} {} {}",
            network_config.bootstrap_url, network_config.signal_url, network_config.relay_url
        );
        assert!(!rendered.contains("holochain.org"), "public endpoint survived: {rendered}");
        assert!(!rendered.contains("iroh.link"), "public endpoint survived: {rendered}");
    }
}
