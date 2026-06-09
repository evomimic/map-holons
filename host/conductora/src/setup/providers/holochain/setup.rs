use crate::config::providers::holochain::{CellDetail, HolochainConfig};
use crate::config::StorageProvider;
use crate::runtime::RuntimeInitiatorState;
use crate::setup::common_setup::{register_receptor, serialize_props};
use crate::setup::providers::holochain::plugins::hc_dev_mode_enabled;
use crate::setup::window_setup::ProviderWindowSetup;
use async_trait::async_trait;
use client_shared_types::base_receptor::ReceptorType;
use client_shared_types::deprecated_base_receptor::DeprecatedBaseReceptor;
use client_shared_types::storage_receptor::{ActiveStorageReceptor, StorageReceptor};
use holochain_client::{AdminWebsocket, AppInfo, AppWebsocket};
use holochain_receptor::{HolochainConductorClient, HolochainReceptor};
use holons_trust_channel::TrustChannel;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Theme};
use tauri_plugin_holochain::AgentPubKey;
use tauri_plugin_holochain::{AppBundle, HolochainExt};

pub struct HolochainSetup;

impl HolochainSetup {
    pub async fn setup(
        handle: AppHandle,
        name: &str,
        provider: &StorageProvider,
    ) -> anyhow::Result<()> {
        let t_setup = std::time::Instant::now();
        tracing::info!("[HOLOCHAIN SETUP] Installing Holochain App.");
        let StorageProvider::Holochain(hc_cfg) = provider else {
            return Err(anyhow::anyhow!("Invalid storage provider config for Holochain"));
        };
        let app_id = &hc_cfg.app_id;
        let dev_mode = hc_dev_mode_enabled();

        let happ = load_happ_bundle(hc_cfg).map_err(|e| {
            tracing::error!("[HOLOCHAIN SETUP] Failed to load happ bundle: {}", e);
            anyhow::anyhow!("Failed to load happ bundle: {}", e)
        })?;
        tracing::debug!(
            "[HOLOCHAIN SETUP] happ bundle loaded in {:.1}s",
            t_setup.elapsed().as_secs_f64()
        );

        let t_admin = std::time::Instant::now();
        let admin_ws = handle.holochain()?.admin_websocket().await?;
        tracing::debug!(
            "[HOLOCHAIN SETUP] Admin websocket obtained in {:.1}s",
            t_admin.elapsed().as_secs_f64()
        );

        let installed_apps = admin_ws
            .list_apps(None)
            .await
            .map_err(tauri_plugin_holochain::Error::ConductorApiError)?;

        let t_install = std::time::Instant::now();
        if dev_mode && Self::is_app_installed(&installed_apps, app_id.clone()) {
            // Dev mode but app is already installed (wipe didn't clear it, e.g. first-ever run
            // or wipe failed). Skip update_app_if_necessary — the bundle store record
            // may not exist for the dev conductor dir, causing a spurious "app not found" error.
            tracing::warn!("[HOLOCHAIN SETUP] Dev mode: app '{}' already installed (wipe may have been skipped on this run). Skipping update check.", app_id);
        } else if dev_mode {
            // Dev mode: conductor state (except wasm.db) wiped before the conductor started
            // (see clean_dev_conductor_state in launch.rs). Install fresh with an ephemeral key.
            Self::handle_new_app_installation(&handle, &admin_ws, happ, app_id.clone(), true)
                .await?;
        } else if Self::is_app_installed(&installed_apps, app_id.clone()) {
            Self::handle_existing_app(&handle, happ, app_id.clone()).await?;
        } else {
            Self::handle_new_app_installation(&handle, &admin_ws, happ, app_id.clone(), false)
                .await?;
        }
        tracing::info!(
            "[HOLOCHAIN SETUP] App install/update done in {:.1}s",
            t_install.elapsed().as_secs_f64()
        );

        let t_appws = std::time::Instant::now();
        let app_ws = handle.holochain()?.app_websocket(app_id.clone()).await?;
        tracing::debug!(
            "[HOLOCHAIN SETUP] App websocket obtained in {:.1}s",
            t_appws.elapsed().as_secs_f64()
        );

        let cell_details = hc_cfg
            .cell_details
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("cell_details missing in HolochainConfig"))?;
        if cell_details.is_empty() {
            return Err(anyhow::anyhow!("cell_details is empty in HolochainConfig"));
        }
        let cell0 = &cell_details[0];
        let agent = app_ws.my_pub_key.clone();

        // New receptor path: signal pipeline + ActiveStorageReceptor.
        // Websocket clones are Arc-backed — all clones share the same conductor connection.
        let receptor = HolochainReceptor::new(
            name.to_string(),
            serialize_props(hc_cfg),
            app_ws.clone(),
            admin_ws.clone(),
            cell0.role_name.clone(),
            cell0.zome_name.clone(),
            cell0.zome_function.clone(),
            agent,
        )
        .await;

        if let Some(state) = handle.try_state::<ActiveStorageReceptor>() {
            *state.write().expect("ActiveStorageReceptor lock poisoned") =
                Some(receptor.clone() as Arc<dyn StorageReceptor>);
        } else {
            tracing::warn!(
                "[HOLOCHAIN SETUP] ActiveStorageReceptor missing; space queries will not be available."
            );
        }

        if let Some(state) = handle.try_state::<RuntimeInitiatorState>() {
            let initiator: Arc<dyn holons_core::dances::DanceInitiator> =
                Arc::new(TrustChannel::new(receptor.client.clone()));
            *state.write().expect("RuntimeInitiatorState lock poisoned") = Some(initiator);
        } else {
            tracing::warn!(
                "[HOLOCHAIN SETUP] RuntimeInitiatorState missing; runtime will not initialize."
            );
        }

        // Deprecated path — kept for legacy DeprecatedHolochainReceptor consumers.
        let (receptor_cfg, _client) =
            Self::build_receptor(app_ws, admin_ws, name, hc_cfg, cell0).await?;
        register_receptor(&handle, receptor_cfg).await?;
        tracing::info!(
            "[HOLOCHAIN SETUP] Total setup time: {:.1}s",
            t_setup.elapsed().as_secs_f64()
        );

        Ok(())
    }

    fn is_app_installed(app_infos: &[AppInfo], app_id: String) -> bool {
        app_infos.iter().any(|app_info| app_info.installed_app_id.as_str() == app_id)
    }

    async fn handle_existing_app(
        handle: &AppHandle,
        happ: AppBundle,
        app_id: String,
    ) -> anyhow::Result<()> {
        let app_ws = handle.holochain()?.app_websocket(app_id.clone()).await?;
        tracing::info!("[HOLOCHAIN SETUP] App '{}' already installed.", app_id);

        handle.holochain()?.update_app_if_necessary(app_id.clone(), happ).await?;

        match app_ws.app_info().await {
            Ok(_) => {
                tracing::info!(
                    "[HOLOCHAIN SETUP] App websocket connected successfully. Agent: {:?}",
                    app_ws.my_pub_key
                );
            }
            Err(e) => {
                tracing::warn!("[HOLOCHAIN SETUP] App websocket connection issue: {:?}", e);
            }
        }

        tracing::debug!("[HOLOCHAIN SETUP] App update check completed for '{}'.", app_id);
        Ok(())
    }

    async fn handle_new_app_installation(
        handle: &AppHandle,
        admin_ws: &AdminWebsocket,
        happ: AppBundle,
        app_id: String,
        dev_mode: bool,
    ) -> anyhow::Result<()> {
        tracing::debug!("[HOLOCHAIN SETUP] App '{}' not found. Installing...", app_id);

        // DangerTestKeystore has no device_seed_lair_tag in dev mode, so the conductor
        // cannot auto-derive an agent key — generate one explicitly.
        let agent_key: Option<AgentPubKey> = if dev_mode {
            let key = admin_ws
                .generate_agent_pub_key()
                .await
                .map_err(|e| anyhow::anyhow!("generate_agent_pub_key failed: {e}"))?;
            tracing::debug!("[HOLOCHAIN SETUP] Dev mode: generated ephemeral agent key {:?}", key);
            Some(key)
        } else {
            None // production: conductor derives key from device_seed_lair_tag
        };

        let appinfo = handle.holochain()?.install_app(app_id, happ, None, agent_key, None).await?;
        tracing::debug!("[HOLOCHAIN SETUP] App installed: {:?}", appinfo);
        Ok(())
    }

    async fn build_receptor(
        app_ws: AppWebsocket,
        admin_ws: AdminWebsocket,
        name: &str,
        hc_cfg: &HolochainConfig,
        cell_detail: &CellDetail,
    ) -> anyhow::Result<(DeprecatedBaseReceptor, Arc<HolochainConductorClient>)> {
        let agent = app_ws.my_pub_key.clone();
        let client = Self::setup_holochain_client(
            app_ws.clone(),
            admin_ws.clone(),
            cell_detail.clone(),
            agent,
        )
        .await;
        let props = serialize_props(hc_cfg);

        let receptor = DeprecatedBaseReceptor {
            receptor_id: name.to_string(),
            receptor_type: ReceptorType::Holochain,
            client_handler: Some(client.clone() as Arc<dyn std::any::Any + Send + Sync>),
            properties: props,
        };

        Ok((receptor, client))
    }

    async fn setup_holochain_client(
        app_ws: AppWebsocket,
        admin_ws: AdminWebsocket,
        cell_detail: CellDetail,
        agent: AgentPubKey,
    ) -> Arc<HolochainConductorClient> {
        Arc::new(HolochainConductorClient {
            app_ws: Arc::new(Mutex::new(Some(app_ws))),
            admin_ws: Arc::new(Mutex::new(Some(admin_ws))),
            rolename: cell_detail.role_name,
            zomename: cell_detail.zome_name,
            zomefunction: cell_detail.zome_function,
            agent,
        })
    }
}

/// Load and validate the happ bundle from the filesystem path specified in config.
pub fn load_happ_bundle(holochain_config: &HolochainConfig) -> anyhow::Result<AppBundle> {
    let happ_relative = holochain_config.happ_path.clone().unwrap_or_else(|| {
        let default = "happ/workdir/map-holons.happ".to_string();
        tracing::warn!("[HAPP LOADER] happ_path not set in config, using default: {}", default);
        default
    });

    tracing::debug!("[HAPP LOADER] Using happ_path: {}", happ_relative);

    // Resolve relative to workspace root, not current_dir (which varies by runner context).
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| anyhow::anyhow!("Failed to determine workspace root"))?;

    let happ_path = workspace_root.join(&happ_relative);
    tracing::debug!("[HAPP LOADER] Resolved path: {:?}", happ_path);

    if !happ_path.exists() {
        return Err(anyhow::anyhow!("Happ file not found at: {:?}", happ_path));
    }

    let bytes = std::fs::read(&happ_path)
        .map_err(|e| anyhow::anyhow!("Failed to read happ file: {}", e))?;
    tracing::debug!("[HAPP LOADER] Loaded {} bytes", bytes.len());

    AppBundle::unpack(std::io::Cursor::new(bytes))
        .map_err(|e| anyhow::anyhow!("Failed to decode happ bundle: {}", e))
}

pub struct HolochainWindowSetup;

#[async_trait]
impl ProviderWindowSetup for HolochainWindowSetup {
    async fn create_window(&self, handle: &AppHandle, app_id: &str) -> anyhow::Result<()> {
        use tauri_plugin_holochain::HolochainExt;

        tracing::debug!("[WINDOW SETUP] Creating holochain window.");

        let main_window_builder = handle
            .holochain()?
            .main_window_builder(String::from("main"), false, Some(app_id.to_string()), None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to build holochain window: {}", e))?;

        let _main_window = main_window_builder
            .inner_size(1280.0, 1040.0)
            .min_inner_size(1100.0, 900.0)
            .theme(Some(Theme::Dark))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build main window: {}", e))?;

        tracing::debug!("[WINDOW SETUP] Holochain window created successfully.");
        Ok(())
    }
}
