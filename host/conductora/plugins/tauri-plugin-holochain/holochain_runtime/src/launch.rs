use std::path::Path;
use std::sync::Arc;

use async_std::sync::Mutex;
use holochain::conductor::{config::ConductorConfig, Conductor};
use keystore::spawn_lair_keystore_in_proc;
// use holochain_keystore::lair_keystore::spawn_lair_keystore_in_proc;
use lair_keystore::dependencies::hc_seed_bundle::SharedLockedArray;

use crate::{filesystem::FileSystem, HolochainRuntime, HolochainRuntimeConfig};

mod config;
mod keystore;
mod mdns;
use mdns::spawn_mdns_bootstrap;

pub const DEVICE_SEED_LAIR_KEYSTORE_TAG: &'static str = "DEVICE_SEED";

/// Write the conductor configuration to a YAML file in the app data directory
/// so that external tooling can discover the conductor's layout on disk.
fn write_conductor_config(
    app_data_dir: &Path,
    conductor_config: &ConductorConfig,
) -> std::io::Result<()> {
    let config_yaml_path = app_data_dir.join("conductor-config.yaml");
    let yaml = serde_yaml::to_string(conductor_config)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    std::fs::write(&config_yaml_path, yaml)?;
    log::info!("Wrote conductor config to {}", config_yaml_path.display());
    Ok(())
}

/// Launch the holochain conductor in the background
pub(crate) async fn launch_holochain_runtime(
    passphrase: SharedLockedArray,
    config: HolochainRuntimeConfig,
) -> crate::error::Result<HolochainRuntime> {
    let t_total: std::time::Instant = std::time::Instant::now();
    let filesystem = FileSystem::new(config.holochain_dir).await?;
    let admin_port = if let Some(admin_port) = config.admin_port {
        admin_port
    } else {
        portpicker::pick_unused_port().expect("No ports free")
    };

    let mut dev_dir = None;

    let network_config = if config.dev_mode {
        dev_dir = Some(config.dev_data_root.clone().expect("dev_mode=true requires dev_data_root"));

        let mut n = config.network_config;
        n.bootstrap_url = url2::url2!("http://127.0.0.1:1");
        n
    } else {
        config.network_config
    };

    let conductor_config = config::conductor_config(
        &filesystem,
        admin_port,
        filesystem.keystore_dir().into(),
        network_config,
        config.dev_mode,
        dev_dir,
    );

    log::debug!("Built conductor config: {:?}.", conductor_config);

    if let Err(err) = write_conductor_config(&filesystem.app_data_dir, &conductor_config) {
        log::error!("Failed to write conductor config to disk: {}", err);
    }

    let conductor_handle = match config.dev_mode {
        true => {
            clean_dev_conductor_state(
                &config.dev_data_root.clone().expect("dev_mode=true requires dev_data_root"),
            );

            Conductor::builder().config(conductor_config).build().await?
        }
        false => {
            let keystore =
                spawn_lair_keystore_in_proc(&filesystem.keystore_config_path(), passphrase.clone())
                    .map_err(|err| crate::Error::LairError(err))?;

            log::info!("Keystore spawned successfully.");

            let seed_already_exists = keystore
                .lair_client()
                .get_entry(DEVICE_SEED_LAIR_KEYSTORE_TAG.into())
                .await
                .is_ok();

            if !seed_already_exists {
                keystore
                    .lair_client()
                    .new_seed(DEVICE_SEED_LAIR_KEYSTORE_TAG.into(), None, true)
                    .await
                    .map_err(|err| crate::Error::LairError(err))?;
            } else {
                log::info!("Device seed already exists in keystore, skipping generation.");
            }

            Conductor::builder()
                .config(conductor_config)
                .passphrase(Some(passphrase))
                .with_keystore(keystore)
                .build()
                .await?
        }
    };

    log::info!("Connected to the admin websocket");

    if config.dev_mode {
        log::warn!("Running in DEV MODE: using in-memory keystore and forcing local-only network config. NOT FOR PRODUCTION USE!");
    } else if config.mdns_discovery {
        spawn_mdns_bootstrap(admin_port).await?;
    }
    tracing::info!(
        "[LAUNCH] Total launch_holochain_runtime: {:.1}s",
        t_total.elapsed().as_secs_f64()
    );

    Ok(HolochainRuntime {
        filesystem,
        apps_websockets_auths: Arc::new(Mutex::new(Vec::new())),
        admin_port,
        conductor_handle,
    })
}

//helper functions

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

    // Ensure the wasm-cache directory exists even after a full wipe.
    // holochain_wasmer_host crashes (SIGSEGV via libunwind on ARM64) when it
    // tries to access a wasm-cache directory that doesn't exist, because the
    // io::Error is treated as a fatal WasmHostError rather than a clean miss.
    let _ = std::fs::create_dir_all(&wasm_cache_dir);

    tracing::warn!(
        "[LAUNCH] DEV MODE: conductor state reset in {:.2}s (wasm cache preserved in-place)",
        t.elapsed().as_secs_f64()
    );
}
