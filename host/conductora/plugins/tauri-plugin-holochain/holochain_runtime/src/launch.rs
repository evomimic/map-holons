use std::sync::Arc;

use async_std::sync::Mutex;
use holochain_keystore::lair_keystore::spawn_lair_keystore_in_proc;
use lair_keystore::dependencies::hc_seed_bundle::SharedLockedArray;
use url2::url2;

use holochain::conductor::Conductor;

use crate::{
    filesystem::FileSystem,
    launch::signal::{can_connect_to_signal_server, run_local_signal_service},
    HolochainRuntime, HolochainRuntimeConfig,
};

mod config;
//mod keystore;
mod mdns;
mod signal;
use mdns::spawn_mdns_bootstrap;

pub const DEVICE_SEED_LAIR_KEYSTORE_TAG: &'static str = "DEVICE_SEED";

/// Launch the holochain conductor in the background
pub(crate) async fn launch_holochain_runtime(
    passphrase: SharedLockedArray,
    config: HolochainRuntimeConfig,
) -> crate::error::Result<HolochainRuntime> {
    let t_total = std::time::Instant::now();
    let hc_dev_mode_raw = std::env::var("HC_DEV_MODE").ok();
    let hc_dev_mode_parsed = hc_dev_mode_raw
        .as_deref()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    tracing::info!(
        "[LAUNCH] Starting holochain runtime (config.dev_mode={}, env.HC_DEV_MODE(raw)={:?}, env.HC_DEV_MODE(parsed)={})",
        config.dev_mode,
        hc_dev_mode_raw,
        hc_dev_mode_parsed
    );
    if rustls::crypto::aws_lc_rs::default_provider().install_default().is_err() {
        tracing::error!("could not set crypto provider for tls");
    }

    let filesystem = FileSystem::new(config.holochain_dir).await?;
    let admin_port = if let Some(admin_port) = config.admin_port {
        admin_port
    } else {
        portpicker::pick_unused_port().expect("No ports free")
    };

    let mut maybe_local_signal_server: Option<(url2::Url2, sbd_server::SbdServer)> = None;
    let dev_mode = config.dev_mode;
    let signal_url_configured = config.signal_url_configured;
    let configured_signal_url = config.network_config.signal_url.clone();
    tracing::debug!(
        "[LAUNCH] Signal preflight: dev_mode={}, signal_url_configured={}, configured_signal_url={}, fallback_to_lan_only={}, dev_data_root_present={}",
        dev_mode,
        signal_url_configured,
        configured_signal_url.as_str(),
        config.fallback_to_lan_only,
        config.dev_data_root.is_some()
    );

    let signal_policy = signal_launch_policy(dev_mode, signal_url_configured);
    tracing::debug!(
        "[LAUNCH] Signal policy selected: {:?} (dev_mode={}, signal_url_configured={})",
        signal_policy,
        dev_mode,
        signal_url_configured
    );

    match signal_policy {
        SignalLaunchPolicy::SkipSignalSetupInDev => {
            tracing::warn!(
                "HOLOCHAIN DEV MODE ENABLED: ephemeral keystore, no signal networking, disposable conductor state"
            );
            tracing::debug!(
                "[LAUNCH] DEV MODE: skipping all signal setup (no local signal server launch, no WAN reachability check)"
            );
        }
        SignalLaunchPolicy::PreferLocalWhenSignalUrlMissing => {
            tracing::info!(
                "[LAUNCH] PRODUCTION MODE: with signal_url missing/null in config: attempting local signal server startup"
            );
            let my_local_ip = get_local_ip_address();
            let port = portpicker::pick_unused_port().expect("No ports free");
            match run_local_signal_service(my_local_ip.to_string(), port).await {
                Ok(signal_handle) => {
                    let local_signal_url = url2!("ws://{my_local_ip}:{port}");
                    tracing::info!(
                        "[LAUNCH] Local signal server started at {} because signal_url is not configured",
                        local_signal_url.as_str()
                    );
                    maybe_local_signal_server = Some((local_signal_url, signal_handle));
                }
                Err(err) => {
                    tracing::warn!(
                        "[LAUNCH] Failed to start local signal server with signal_url missing/null ({err:?}); continuing with configured/default signal URL {}",
                        configured_signal_url.as_str()
                    );
                }
            }
        }
        SignalLaunchPolicy::CheckWanWhenSignalUrlConfigured => {
            tracing::info!(
                "[LAUNCH] PRODUCTION MODE: with configured signal_url={}; checking WAN signal server reachability",
                configured_signal_url.as_str()
            );
            let connect_result = can_connect_to_signal_server(configured_signal_url.clone()).await;
            tracing::debug!("[LAUNCH] WAN signal check complete.");

            let run_local_signal_server = if let Err(err) = connect_result {
                tracing::warn!("Error connecting with the WAN signal server: {err:?}");
                if config.fallback_to_lan_only {
                    tracing::warn!(
                        "[LAUNCH] fallback_to_lan_only=true; attempting local signal server fallback"
                    );
                    true
                } else {
                    tracing::debug!(
                        "[LAUNCH] fallback_to_lan_only=false; continuing without local signal fallback"
                    );
                    false
                }
            } else {
                tracing::debug!(
                    "[LAUNCH] WAN signal server is reachable; local fallback not needed"
                );
                false
            };

            if run_local_signal_server {
                let my_local_ip = get_local_ip_address();
                let port = portpicker::pick_unused_port().expect("No ports free");
                match run_local_signal_service(my_local_ip.to_string(), port).await {
                    Ok(signal_handle) => {
                        let local_signal_url = url2!("ws://{my_local_ip}:{port}");
                        tracing::info!(
                            "[LAUNCH] Local signal fallback server started at {}",
                            local_signal_url.as_str()
                        );
                        maybe_local_signal_server = Some((local_signal_url, signal_handle));
                    }
                    Err(err) => {
                        tracing::warn!(
                            "[LAUNCH] Failed to start local signal fallback server ({err:?}); continuing with configured signal URL {}",
                            configured_signal_url.as_str()
                        );
                    }
                }
            }
        }
    }

    let local_signal_url_for_config = maybe_local_signal_server.as_ref().map(|s| s.0.clone());
    if dev_mode && local_signal_url_for_config.is_none() {
        tracing::info!(
            "[LAUNCH] DEV MODE: no local signal server handle; conductor_config will use local-only placeholder URLs"
        );
    }
    let effective_signal_url = if let Some(local_signal_url) = local_signal_url_for_config.as_ref()
    {
        local_signal_url.as_str().to_string()
    } else if dev_mode {
        "ws://127.0.0.1:1 (dev placeholder)".to_string()
    } else {
        configured_signal_url.as_str().to_string()
    };
    tracing::debug!(
        "[LAUNCH] Effective signal policy: mode={}, signal_url_configured={}, local_signal_server_running={}, configured_signal_url={}, effective_signal_url={}",
        if dev_mode { "dev" } else { "normal" },
        signal_url_configured,
        maybe_local_signal_server.is_some(),
        configured_signal_url.as_str(),
        effective_signal_url
    );

    let conductor_config = config::conductor_config(
        &filesystem,
        admin_port,
        filesystem.keystore_dir().into(),
        config.network_config,
        local_signal_url_for_config,
        dev_mode,
        config.dev_data_root.clone(),
    );

    tracing::info!("[LAUNCH] Building Conductor...");
    let t1 = std::time::Instant::now();
    let conductor_handle = if dev_mode {
        tracing::debug!(
            "[LAUNCH] DEV MODE conductor path selected; dev_data_root={:?}",
            config.dev_data_root
        );
        // Before starting the conductor, wipe the dev conductor dir so every
        // restart gets a consistent clean slate (no stale WAL/SHM/schema
        // mismatches from the previous run).  wasm.db is saved to a sidecar
        // and restored after the wipe so WASM recompilation is only paid once.
        //
        // inside Nix shells TMPDIR is session-specific
        // (/tmp/nix-shell.XXXX/) so std::env::temp_dir() changes between runs.
        let dev_dir = config.dev_data_root.clone().expect("dev_mode=true requires dev_data_root");
        clean_dev_conductor_state(&dev_dir);

        // DangerTestKeystore is set in the config; no lair process needed.
        Conductor::builder().config(conductor_config).build().await?
    } else {
        tracing::info!("[LAUNCH] Spawning lair keystore (in-proc)...");
        let t0 = std::time::Instant::now();
        let keystore =
            spawn_lair_keystore_in_proc(&filesystem.keystore_config_path(), passphrase.clone())
                .await
                .map_err(|err| crate::Error::LairError(err))?;
        tracing::info!("[LAUNCH] Lair keystore ready in {:.1}s", t0.elapsed().as_secs_f64());

        let seed_already_exists =
            keystore.lair_client().get_entry(DEVICE_SEED_LAIR_KEYSTORE_TAG.into()).await.is_ok();

        if !seed_already_exists {
            keystore
                .lair_client()
                .new_seed(DEVICE_SEED_LAIR_KEYSTORE_TAG.into(), None, true)
                .await
                .map_err(|err| crate::Error::LairError(err))?;
        }

        Conductor::builder()
            .config(conductor_config)
            .passphrase(Some(passphrase))
            .with_keystore(keystore)
            .build()
            .await?
    };
    tracing::info!("[LAUNCH] Conductor ready in {:.1}s", t1.elapsed().as_secs_f64());

    if dev_mode {
        // Dev mode: mDNS peer discovery is irrelevant for single-node CRUD testing; skip it.
        tracing::debug!("[LAUNCH] DEV MODE: skipping mDNS bootstrap");
    } else {
        spawn_mdns_bootstrap(admin_port).await?;
    }

    // *lock = Some(info.clone());

    tracing::info!(
        "[LAUNCH] Total launch_holochain_runtime: {:.1}s",
        t_total.elapsed().as_secs_f64()
    );
    Ok(HolochainRuntime {
        filesystem,
        apps_websockets_auths: Arc::new(Mutex::new(Vec::new())),
        admin_port,
        conductor_handle,
        _local_sbd_server: maybe_local_signal_server.map(|s| s.1),
    })
}

//helper functions

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SignalLaunchPolicy {
    SkipSignalSetupInDev,
    PreferLocalWhenSignalUrlMissing,
    CheckWanWhenSignalUrlConfigured,
}

fn signal_launch_policy(dev_mode: bool, signal_url_configured: bool) -> SignalLaunchPolicy {
    if dev_mode {
        SignalLaunchPolicy::SkipSignalSetupInDev
    } else if signal_url_configured {
        SignalLaunchPolicy::CheckWanWhenSignalUrlConfigured
    } else {
        SignalLaunchPolicy::PreferLocalWhenSignalUrlMissing
    }
}

fn get_local_ip_address() -> std::net::IpAddr {
    // Method 1: Try local_ip_address crate
    if let Ok(ip) = local_ip_address::local_ip() {
        tracing::debug!("Got local IP via local_ip_address crate: {}", ip);
        return ip;
    }

    // Method 2: Try connecting to determine route
    if let Ok(ip) = try_connect_method() {
        tracing::debug!("Got local IP via connect method: {}", ip);
        return ip;
    }

    // Method 3: Parse network interfaces manually
    if let Ok(ip) = try_interface_method() {
        tracing::debug!("Got local IP via interface method: {}", ip);
        return ip;
    }

    // Method 4: Ultimate fallback - localhost
    tracing::warn!("Could not determine local IP, using localhost");
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
}

fn try_connect_method() -> Result<std::net::IpAddr, Box<dyn std::error::Error>> {
    use std::net::UdpSocket; //SocketAddr

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("1.1.1.1:80")?; // Cloudflare DNS
    Ok(socket.local_addr()?.ip())
}

fn try_interface_method() -> Result<std::net::IpAddr, Box<dyn std::error::Error>> {
    // You might need to add `pnet` or `if-addrs` crate for this
    // For now, just return an error to fall through to localhost
    Err("Interface method not implemented".into())
}

/// Wipe the dev conductor directory to get a clean state on every restart,
/// while preserving the compiled WASM cache across restarts so that WASM
/// recompilation (~6s) only happens on the true first cold start.
///
/// Holochain's actual directory layout under `data_root_path`:
///
/// ```text
/// /tmp/conductora_dev/
///   databases/
///     db.key              ← encryption key (shared by all DBs — MUST preserve)
///     wasm/wasm           ← WASM source/bytecode DB
///     conductor/conductor ← conductor state DB (stale agent keys — DELETE)
///     cache/<dna-hash>    ← per-DNA cache (DELETE)
///     dht/<dna-hash>      ← per-DNA DHT  (DELETE)
///     authored/<cell-id>* ← per-cell authored chain (DELETE)
///     p2p/*               ← p2p peer metadata (DELETE)
///   wasm-cache/
///     <hash>              ← ~20 MB pre-compiled WASM module (MUST preserve)
/// ```
///
/// Strategy: selectively delete everything EXCEPT:
///   - `databases/db.key`  — encryption key needed to open wasm DB
///   - `databases/wasm/`   — WASM source/bytecode SQLite DB
///   - `wasm-cache/`       — pre-compiled native WASM modules
///
/// Result: conductor boots with zero installed apps (fresh agent key) but with
/// precompiled WASM, so `install_app` costs ~0.5s instead of ~6s.
fn clean_dev_conductor_state(conductor_dir: &std::path::Path) {
    if !conductor_dir.exists() {
        tracing::warn!("[LAUNCH] DEV MODE: conductor dir does not exist yet — cold start");
        return;
    }

    let t = std::time::Instant::now();
    let db_dir = conductor_dir.join("databases");
    let wasm_db_path = db_dir.join("wasm").join("wasm");
    let wasm_cache_dir = conductor_dir.join("wasm-cache");

    // Check whether the WASM cache artifacts exist from a previous run.
    // Holochain encrypts its SQLite DBs with databases/db.key, so we cannot
    // open them with plain rusqlite to checkpoint the WAL.  Instead we just
    // preserve the files in-place and let holochain replay its own WAL on the
    // next open.
    if wasm_db_path.exists() && wasm_cache_dir.exists() {
        tracing::warn!(
            "[LAUNCH] DEV MODE: wasm DB + wasm-cache found — warm start, WASM should be cached"
        );
    } else {
        tracing::warn!("[LAUNCH] DEV MODE: no existing wasm cache — cold start, WASM will compile");
    }

    // Step 2: Selectively delete stale state, preserving WASM cache artifacts.
    //
    // Inside `databases/` keep `db.key` and `wasm/` — delete everything else
    // (conductor/, cache/, dht/, authored/, p2p/).
    //
    // At the top level keep `wasm-cache/` — delete everything else
    // (keystore dir, bundle store, etc.).
    match std::fs::read_dir(conductor_dir) {
        Err(err) => {
            tracing::warn!("[LAUNCH] DEV MODE: cannot read conductor dir: {err}");
            return;
        }
        Ok(top_entries) => {
            for entry in top_entries.flatten() {
                let path = entry.path();
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if name_str == "wasm-cache" {
                    continue; // keep compiled WASM modules
                }

                if name_str == "databases" && db_dir.is_dir() {
                    // Inside databases/ keep only db.key and wasm/.
                    if let Ok(db_entries) = std::fs::read_dir(&db_dir) {
                        for db_entry in db_entries.flatten() {
                            let db_name = db_entry.file_name();
                            let db_name_str = db_name.to_string_lossy();
                            if db_name_str == "db.key" || db_name_str == "wasm" {
                                continue; // keep
                            }
                            let p = db_entry.path();
                            if p.is_dir() {
                                let _ = std::fs::remove_dir_all(&p);
                            } else {
                                let _ = std::fs::remove_file(&p);
                            }
                        }
                    }
                } else {
                    if path.is_dir() {
                        let _ = std::fs::remove_dir_all(&path);
                    } else {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }

    tracing::warn!(
        "[LAUNCH] DEV MODE: conductor state reset in {:.2}s (wasm cache preserved in-place)",
        t.elapsed().as_secs_f64()
    );
}
