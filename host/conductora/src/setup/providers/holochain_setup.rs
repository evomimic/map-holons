use std::sync::{Arc, Mutex};
use crate::config::providers::holochain::*;
use crate::config::{StorageProvider};
//use crate::APP_ID;
use holons_client::shared_types::base_receptor::BaseReceptor;
use tauri::{AppHandle, Manager, Theme};
use holochain_client::{AdminWebsocket, AppWebsocket, AppInfo};
use holochain_receptor::HolochainConductorClient;
use crate::setup::receptor_config_registry::ReceptorConfigRegistry;
use tauri_plugin_holochain::{HolochainExt, AppBundle};
use async_trait::async_trait;
use crate::setup::window_setup::ProviderWindowSetup;
use tauri_plugin_holochain::AgentPubKey;


pub struct HolochainSetup;

impl HolochainSetup {
    /// Main setup function for Holochain integration
    pub async fn setup(handle: AppHandle, provider: &StorageProvider) -> anyhow::Result<()> {
        tracing::debug!("[HOLOCHAIN SETUP] Starting Holochain setup.");
        let StorageProvider::Holochain(hc_cfg) = provider else {
            return Err(anyhow::anyhow!("Invalid storage provider config for Holochain"));
        };
        let app_id = &hc_cfg.app_id;

        // Load and validate happ bundle early
        let happ = match load_happ_bundle(hc_cfg) {
            Ok(bundle) => bundle,
            Err(e) => {
                tracing::error!("[HOLOCHAIN SETUP] Failed to load happ bundle: {}", e);
                return Err(anyhow::anyhow!("Failed to load happ bundle: {}", e));
            }
        };
        
        let admin_ws = handle.holochain()?.admin_websocket().await?;
        tracing::debug!("[HOLOCHAIN SETUP] Admin websocket obtained.");

        let installed_apps = admin_ws
            .list_apps(None)
            .await
            .map_err(|err| tauri_plugin_holochain::Error::ConductorApiError(err))?;

        if Self::is_app_installed(&installed_apps, app_id.clone()) {
            Self::handle_existing_app(&handle, happ, app_id.clone()).await?;
        } else {
            Self::handle_new_app_installation(&handle, happ, app_id.clone()).await?;
        }

        let app_ws = handle.holochain()?.app_websocket(app_id.clone()).await?;
        tracing::debug!("[HOLOCHAIN SETUP] App websocket obtained.");

        // After successful setup, build and register the receptor
        let receptor_cfg: BaseReceptor = Self::build_receptor(app_ws, admin_ws, hc_cfg).await?;
        Self::register_receptor(&handle, receptor_cfg).await?;

        Ok(())
    }

    /// Check if the app is already installed
    fn is_app_installed(app_infos: &[AppInfo], app_id:String) -> bool {
        app_infos
            .iter()
            .any(|app_info| app_info.installed_app_id.as_str() == app_id)
    }

    /// Handle setup for existing app installation
    async fn handle_existing_app(
        handle: &AppHandle,
        happ: AppBundle,
        app_id:String,
    ) -> anyhow::Result<()> {

        let app_ws = handle.holochain()?.app_websocket(app_id.clone()).await?;
        tracing::info!("[HOLOCHAIN SETUP] App '{}' already installed.", app_id.clone());

        handle.holochain()?.update_app_if_necessary(
            app_id.clone(),
            happ
        ).await?;
                
        // Verify connection
        match app_ws.app_info().await {
            Ok(_app_info) => {
                tracing::info!("[HOLOCHAIN SETUP] App websocket connected successfully. Agent: {:?}", 
                          app_ws.my_pub_key);
            },
            Err(e) => {
                tracing::warn!("[HOLOCHAIN SETUP] App websocket connection issue: {:?}", e);
            }
        }

        tracing::debug!("[HOLOCHAIN SETUP] App update check completed for '{}'.", app_id);
        Ok(())
    }

    /// Handle new app installation
    async fn handle_new_app_installation(
        handle: &AppHandle,
        happ: AppBundle,
        app_id:String
    ) -> anyhow::Result<()> {
        tracing::info!("[HOLOCHAIN SETUP] App '{}' not found. Installing...", app_id.clone());
        
        let appinfo = handle
            .holochain()?
            .install_app(
                app_id,
                happ,
                None,
                None,
                None,
            )
            .await?;
        
        tracing::debug!("[HOLOCHAIN SETUP] App installed: {:?}", appinfo);
        Ok(())
    }

    async fn build_receptor(
        app_ws: AppWebsocket,
        admin_ws: AdminWebsocket,
        hc_cfg: &HolochainConfig
    ) -> anyhow::Result<BaseReceptor> {
            
            let agent = app_ws.my_pub_key.clone();
            let cell_details = hc_cfg.cell_details.as_ref().ok_or_else(|| anyhow::anyhow!("cell_details missing in HolochainConfig"))?;
            if cell_details.is_empty() {
                return Err(anyhow::anyhow!("cell_details is empty in HolochainConfig"));
            }
            let client = Self::setup_holochain_client(app_ws.clone(), admin_ws.clone(), cell_details[0].clone(), agent).await;
            
            // Dynamically collect all properties from HolochainConfig
            let props = match serde_json::to_value(hc_cfg)? {
                serde_json::Value::Object(map) => {
                    map.into_iter()
                        .map(|(k, v)| {
                            let value_str = match v {
                                serde_json::Value::String(s) => s,
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                serde_json::Value::Null => String::new(),
                                _ => v.to_string(),
                            };
                            (k, value_str)
                        })
                        .collect::<std::collections::HashMap<String, String>>()
                }
                _ => std::collections::HashMap::new(),
            };

            return Ok(BaseReceptor {
                receptor_id: None,
                receptor_type: "holochain".to_string(),
                client_handler: Some(client),
                properties: props,
            });
        }

        /// Initialize the receptor factory with websockets and load configuration
        /// Register the built receptor config into the application state
        async fn register_receptor(
            handle: &AppHandle,
            receptor_cfg: BaseReceptor,
        ) -> anyhow::Result<()> {
            // Get the registry from app state and register the new config
            let registry = handle.state::<ReceptorConfigRegistry>();
            registry.register(receptor_cfg);
            Ok(())
        }

        //TODO: this should be done by the receptor setup code (basereceptor properties) and include ROLENAME, ZOMENAME etc
        pub async fn setup_holochain_client(
            app_ws: AppWebsocket,
            admin_ws: AdminWebsocket,
            cell_detail: CellDetail,
            agent: AgentPubKey,
            //cell_id: CellId,
        ) -> Arc<HolochainConductorClient> {

            let app_ws_arc = Arc::new(Mutex::new(Some(app_ws)));
            let admin_ws_arc = Arc::new(Mutex::new(Some(admin_ws)));
            let rolename = cell_detail.role_name;
            let zomename = cell_detail.zome_name;
            let zomefunction = cell_detail.zome_function;

            Arc::new(HolochainConductorClient {
                app_ws: app_ws_arc,
                admin_ws: admin_ws_arc,
                rolename,
                zomename,
                zomefunction,
                agent,
                //cell_id,
            })
        }

}

// Holochain window setup
pub struct HolochainWindowSetup;

#[async_trait]
impl ProviderWindowSetup for HolochainWindowSetup {

    async fn create_window(&self, handle: &AppHandle, app_id: &str) -> anyhow::Result<()> {
        use tauri_plugin_holochain::HolochainExt;
        
        tracing::debug!("[WINDOW SETUP] Creating holochain window.");
        
        let main_window_builder = handle
            .holochain()?
            //.ok_or(anyhow::anyhow!("Holochain plugin not available"))?
            .main_window_builder(
                String::from("main"), 
                false, 
                Some(app_id.to_string()), 
                None
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to build holochain window: {}", e))?;
        
        let _main_window = main_window_builder
            .theme(Some(Theme::Dark))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build main window: {}", e))?;
            
        tracing::debug!("[WINDOW SETUP] Holochain window created successfully.");
        Ok(())
    }
}