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

// pub static RUNNING_HOLOCHAIN: RwLock<Option<RunningHolochainInfo>> = RwLock::const_new(None);

/// Launch the holochain conductor in the background
pub(crate) async fn launch_holochain_runtime(
    passphrase: SharedLockedArray,
    config: HolochainRuntimeConfig,
) -> crate::error::Result<HolochainRuntime> {
    let t_total = std::time::Instant::now();
    tracing::info!("[LAUNCH] Starting holochain runtime (dev_mode={})", config.dev_mode);
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

    if config.dev_mode {
        // Dev mode: skip the WAN signal check entirely (avoids 1-5s TCP handshake/timeout).
        // Spin up an instant loopback signal server instead — the conductor needs *some*
        // signal URL in its network config, but it never actually needs to reach the internet.
        tracing::info!(
            "[LAUNCH] DEV MODE: skipping WAN signal check, starting local loopback signal server"
        );
        let loopback = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
        let port = portpicker::pick_unused_port().expect("No ports free");
        let signal_handle = run_local_signal_service(loopback.to_string(), port).await?;
        let local_signal_url = url2!("ws://{loopback}:{port}");
        maybe_local_signal_server = Some((local_signal_url, signal_handle));
    } else {
        tracing::info!("[LAUNCH] Checking WAN signal server reachability…");
        let connect_result =
            can_connect_to_signal_server(config.network_config.signal_url.clone()).await;
        tracing::info!("[LAUNCH] WAN signal check complete.");

        let run_local_signal_server = if let Err(err) = connect_result {
            tracing::warn!("Error connecting with the WAN signal server: {err:?}");
            config.fallback_to_lan_only
        } else {
            false
        };

        if run_local_signal_server {
            let my_local_ip = get_local_ip_address();
            let port = portpicker::pick_unused_port().expect("No ports free");
            let signal_handle = run_local_signal_service(my_local_ip.to_string(), port).await?;
            let local_signal_url = url2!("ws://{my_local_ip}:{port}");
            maybe_local_signal_server = Some((local_signal_url, signal_handle));
        }
    }

    let dev_mode = config.dev_mode;

    let conductor_config = config::conductor_config(
        &filesystem,
        admin_port,
        filesystem.keystore_dir().into(),
        config.network_config,
        maybe_local_signal_server.as_ref().map(|s| s.0.clone()),
        dev_mode,
    );

    tracing::info!("[LAUNCH] Building Conductor (DB migration + networking)...");
    let t1 = std::time::Instant::now();
    let conductor_handle = if dev_mode {
        // Before starting the conductor, wipe the dev conductor dir so every
        // restart gets a consistent clean slate (no stale WAL/SHM/schema
        // mismatches from the previous run).  wasm.db is saved to a sidecar
        // and restored after the wipe so WASM recompilation is only paid once.
        //
        // IMPORTANT: must match config.rs dev_mode data_root_path.
        // Use a hardcoded absolute path — inside Nix shells TMPDIR is session-specific
        // (/tmp/nix-shell.XXXX/) so std::env::temp_dir() changes between runs.
        let dev_dir = std::path::PathBuf::from("/tmp/conductora_dev");
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

    tracing::info!("Connected to the admin websocket");

    if dev_mode {
        // Dev mode: mDNS peer discovery is irrelevant for single-node CRUD testing; skip it.
        tracing::info!("[LAUNCH] DEV MODE: skipping mDNS bootstrap");
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

fn get_local_ip_address() -> std::net::IpAddr {
    // Method 1: Try local_ip_address crate
    if let Ok(ip) = local_ip_address::local_ip() {
        tracing::info!("Got local IP via local_ip_address crate: {}", ip);
        return ip;
    }

    // Method 2: Try connecting to determine route
    if let Ok(ip) = try_connect_method() {
        tracing::info!("Got local IP via connect method: {}", ip);
        return ip;
    }

    // Method 3: Parse network interfaces manually
    if let Ok(ip) = try_interface_method() {
        tracing::info!("Got local IP via interface method: {}", ip);
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
