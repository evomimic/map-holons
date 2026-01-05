use crate::{
    map_commands as commands,
    config::{
        app_config::load_storage_config, providers::holochain::holochain_plugin, storage_config::{StorageConfig, StorageProvider}
    },
    setup::{
        holochain_setup::{HolochainSetup, HolochainWindowSetup}, local_setup::LocalSetup, window_setup::DefaultWindowSetup},
};

use crate::setup::window_setup::ProviderWindowSetup;
use crate::setup::receptor_config_registry::ReceptorConfigRegistry;
use holons_receptor::ReceptorFactory;
use tauri::{AppHandle, Manager, Listener};

pub struct AppBuilder;

impl AppBuilder {
    /// Build and configure the Tauri application
    pub fn build() -> tauri::Builder<tauri::Wry> {
        tracing::debug!("[APP BUILDER] Setting up Tauri application.");
        // Load storage config once and store in state
        let storage_cfg = load_storage_config();
        // Base builder without setup
        let base = tauri::Builder::default()
            .manage(storage_cfg.clone())
            .manage(ReceptorFactory::new())
            .manage(ReceptorConfigRegistry::new())
            .invoke_handler(tauri::generate_handler![
                commands::root_space,
                //commands::load_holons,
                commands::serde_test,
                commands::map_request,
                commands::all_spaces,
                commands::is_service_ready,
            ]);
        // First apply provider-specific plugins
        let with_plugins = Self::apply_plugins(base, &storage_cfg);
        // Then register the common setup handler
        with_plugins.setup(Self::setup_handler)
    }

    /// Setup handler for application initialization
    fn setup_handler(app: &mut tauri::App<tauri::Wry>) -> Result<(), Box<dyn std::error::Error>> {
        tracing::debug!("[APP BUILDER] Tauri setup closure executing.");

        let handle = app.handle().clone();
        let storage_cfg = app.state::<StorageConfig>().inner().clone();
        tracing::debug!("[APP BUILDER] Storage config: {:#?}", storage_cfg);

        let enabled_providers = Self::get_enabled_provider_types(&storage_cfg);
        if enabled_providers.contains(&"holochain") {
            tracing::debug!("[APP BUILDER] Holochain provider detected, waiting for setup completion.");
            app.handle().listen("holochain://setup-completed", move |_event| {
                tracing::debug!("[APP BUILDER] Received 'holochain://setup-completed' event.");
                let handle = handle.clone();
                let storage_cfg = storage_cfg.clone(); // ← Clone for the closure
                tauri::async_runtime::spawn(async move {
                    Self::run_complete_setup(&handle, &storage_cfg).await;
                });
            });
        } else {
            tracing::info!("[APP BUILDER] No async provider setup required.");
            tauri::async_runtime::spawn(async move {
                Self::run_complete_setup(&handle, &storage_cfg).await;
            });
        }

        Ok(())
    }


    /// Run the complete setup: provider setup → load receptors → create window
    async fn run_complete_setup(handle: &AppHandle, storage_cfg: &StorageConfig) {
        tracing::debug!("[APP BUILDER] Running complete setup.");

        // Run provider-specific setup routines
        if let Err(e) = Self::apply_setups(handle).await {
            tracing::error!("[APP BUILDER] Provider setup failed: {}", e);
        }

        // Load receptor configs into factory
        if let Err(e) = Self::load_receptor_configs(handle).await {
            tracing::error!("[APP BUILDER] Failed to load receptor configs: {}", e);
            return;
        }

        // Create main window
        if let Err(e) = Self::create_window(handle, storage_cfg).await {
            tracing::error!("[APP BUILDER] Window creation failed: {}", e);
            return; // ← This is fine since function returns ()
        }

        tracing::info!("[APP BUILDER] Setup completed successfully.");
    }

    /// Load receptor configs from registry into factory
    async fn load_receptor_configs(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(registry) = handle.try_state::<ReceptorConfigRegistry>() {
            let configs = registry.all();
            if let Some(factory) = handle.try_state::<ReceptorFactory>() {
                factory.load_from_configs(configs).await?;
                tracing::debug!("[APP BUILDER] ReceptorFactory loaded from configs.");
            }
        }
        Ok(())
    }

    /// Apply provider-specific plugins based on the storage configuration
    fn apply_plugins(
        mut builder: tauri::Builder<tauri::Wry>,
        storage_cfg: &StorageConfig,
    ) -> tauri::Builder<tauri::Wry> {
        tracing::debug!("[APP BUILDER] Loading provider plugins: {:?}", storage_cfg.get_enabled_providers());

        builder = builder.plugin(tauri_plugin_fs::init());

        for (_name, provider) in storage_cfg.get_enabled_providers() {
            match provider.provider_type() {
                "local" => {
                    //tracing::info!("[APP BUILDER] Loading Local storage plugins");
                    // Local storage
                }
                "holochain" => {
                    match holochain_plugin(provider.clone()) {
                        Ok(plugin) => {
                            tracing::info!("[APP BUILDER] Loaded Holochain plugin");
                            builder = builder.plugin(plugin);
                        }
                        Err(e) => {
                            tracing::error!("[APP BUILDER] Failed to load Holochain plugin: {}", e);
                        }
                    }
                }

                "ipfs" => {
                    //tracing::info!("[APP BUILDER] Loading IPFS plugin");
                    // builder = builder.plugin(ipfs_plugin());
                }
                provider_type => {
                    tracing::warn!("[APP BUILDER] Unknown provider type: {}", provider_type);
                }
            }
        }
        builder
    }

    /// Run provider-specific setup routines for each enabled provider
    async fn apply_setups(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        let storage_cfg = handle.try_state::<StorageConfig>()
            .ok_or("Missing StorageConfig in state")?;

        for (_name, provider) in storage_cfg.get_enabled_providers() {
            match provider.provider_type() {
                "local" => {
                    tracing::info!("[APP BUILDER] Running Local storage setup");
                    LocalSetup::setup(handle.clone(), provider).await?;
                }
                "holochain" => {
                    tracing::info!("[APP BUILDER] Running Holochain setup");
                    HolochainSetup::setup(handle.clone(), provider).await?;
                }
                "ipfs" => {
                    //tracing::info!("[APP BUILDER] Running IPFS setup");
                    // IpfsSetup::setup(handle.clone()).await?;
                }
                provider_type => {
                    tracing::warn!("[APP BUILDER] Unknown provider type for setup: {}", provider_type);
                }
            }
        }
        Ok(())
    }

    async fn create_window(handle: &AppHandle, storage_cfg: &StorageConfig) -> anyhow::Result<()> {
        // Check if window already exists
        if handle.get_webview_window("main").is_some() {
            tracing::info!("[APP BUILDER] Main window already exists, skipping creation.");
            return Ok(());
        }

        let enabled_providers = Self::get_enabled_provider_types(storage_cfg); // ← Use helper
        if enabled_providers.contains(&"holochain") {
            let hc_provider = Self::get_provider_config(storage_cfg, "holochain")?;
            let h_cfg = match hc_provider {
                StorageProvider::Holochain(cfg) => cfg,
                _ => return Err(anyhow::anyhow!("Invalid storage provider config for Holochain")),
            };
            let appid = h_cfg.app_id.clone();

            tracing::info!("[APP BUILDER] Creating Holochain window {}", appid);
            let setup = HolochainWindowSetup;
            setup.create_window(handle, &appid).await?;
        } else {
            tracing::info!("[APP BUILDER] Creating default window");
            let setup = DefaultWindowSetup;
            setup.create_window(handle, "").await?;
        }

        Ok(())
    }

    /// Helper function to get enabled provider types
    fn get_enabled_provider_types(storage_cfg: &StorageConfig) -> Vec<&str> {
        storage_cfg
            .get_enabled_providers()
            .iter()
            .map(|(_, p)| p.provider_type())
            .collect()
    }
    fn get_provider_config(
        storage_cfg: &StorageConfig,
        provider_type: &str,
    ) -> anyhow::Result<StorageProvider> {
        storage_cfg
            .get_provider(provider_type)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("{} provider not found in config", provider_type))
    }



}