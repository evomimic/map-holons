//! Trusted in-process conductor transport for packaged first-cell provisioning.
use crate::{config::providers::holochain::HolochainConfig, runtime::RuntimeInitiatorState};
use core_types::HolonError;
use holochain_client::CellInfo;
use holons_boundary::envelopes::{DanceRequestEnvelope, DanceResponseEnvelope};
use holons_trust_channel::{DanceEnvelopeTransport, TrustChannel};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_plugin_holochain::{HolochainExt, InstallAppPayload, NativeZomeCaller};

#[derive(Debug)]
struct NativeDanceTransport(NativeZomeCaller);

#[async_trait::async_trait]
impl DanceEnvelopeTransport for NativeDanceTransport {
    async fn initiate_dance_envelope(
        &self,
        envelope: DanceRequestEnvelope,
    ) -> Result<DanceResponseEnvelope, HolonError> {
        self.0.call(envelope).await.map_err(|e| HolonError::ConductorError(e.to_string()))
    }
}

pub async fn setup(handle: &AppHandle, config: &HolochainConfig) -> anyhow::Result<()> {
    let runtime = &handle.holochain()?.holochain_runtime;
    anyhow::ensure!(runtime.backend_only, "Packaged startup requires native-only conductor access");
    handle
        .state::<crate::setup::application_launcher::ApplicationSessionState>()
        .mark_activating_holochain_app()
        .map_err(anyhow::Error::msg)?;
    let caller = initialize_app(
        runtime,
        &config.app_id,
        &handle.path().resource_dir()?.join("resources/map-holons.happ"),
    )
    .await?;
    *handle
        .state::<RuntimeInitiatorState>()
        .write()
        .map_err(|e| anyhow::anyhow!(e.to_string()))? =
        Some(Arc::new(TrustChannel::new(Arc::new(NativeDanceTransport(caller)))));
    Ok(())
}

/// Fixed app/role/cell admission shared by startup and the native-boundary test.
async fn initialize_app(
    runtime: &tauri_plugin_holochain::HolochainRuntime,
    app_id: &str,
    happ_path: &std::path::Path,
) -> anyhow::Result<NativeZomeCaller> {
    let admin = runtime.native_administration();
    let apps = admin.list_apps().await?;
    anyhow::ensure!(apps.len() <= 1, "Packaged startup supports one installed application");
    let existing = apps.into_iter().next();
    if let Some(app) = &existing {
        anyhow::ensure!(app.installed_app_id == app_id, "Unexpected installed application");
    } else {
        let agent = admin.generate_agent_pub_key().await?;
        admin
            .install_app(InstallAppPayload {
                agent_key: Some(agent),
                roles_settings: None,
                network_seed: None,
                source: tauri_plugin_holochain::AppBundleSource::Path(happ_path.to_path_buf()),
                installed_app_id: Some(app_id.to_string()),
                ignore_genesis_failure: false,
                restore_from_dht: false,
            })
            .await?;
    }
    let app = admin
        .list_apps()
        .await?
        .into_iter()
        .find(|app| app.installed_app_id == app_id)
        .ok_or_else(|| anyhow::anyhow!("Packaged app is missing after installation"))?;
    admin.verify_packaged_app(&app, happ_path).await?;
    let app = admin.enable_app(app_id.to_string()).await?;
    let cells: Vec<_> = app.cell_info.values().flatten().collect();
    anyhow::ensure!(
        app.cell_info.len() == 1 && app.cell_info.contains_key("map_holons") && cells.len() == 1,
        "Packaged startup requires exactly the approved role/cell"
    );
    let CellInfo::Provisioned(cell) = cells[0] else {
        anyhow::bail!("Cloned and deferred cells are outside the supported configuration")
    };
    Ok(runtime.native_dance_caller(cell.cell_id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_types::ToBaseValue;
    use holons_core::reference_layer::ReadableHolon;
    use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
    use map_commands_runtime::{ExecutionPolicy, Runtime, RuntimeSession};
    use tauri_plugin_holochain::{
        vec_to_locked, AllowedOrigins, HolochainRuntime, HolochainRuntimeConfig, NetworkConfig,
    };

    /// Real persistent conductor and packaged hApp; no test keystore, probes or external ingress.
    /// This is a mechanism test, not a packaged WebView/security-assurance test.
    #[tokio::test(flavor = "multi_thread")]
    async fn native_only_provisioning_retains_identity_without_external_interfaces() {
        let root = std::env::temp_dir().join(format!(
            "map-native-boundary-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let resources = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
        let happ = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../happ/workdir/map-holons.happ");
        let configuration = || {
            let mut network = NetworkConfig::default();
            network.bootstrap_url = url2::Url2::parse("http://127.0.0.1:1");
            network.relay_url = url2::Url2::parse("https://127.0.0.1:1");
            network.target_arc_factor = 0;
            HolochainRuntimeConfig::new(root.clone(), network).backend_only(true)
        };
        let conductor =
            HolochainRuntime::launch(vec_to_locked(vec![]), configuration()).await.unwrap();
        assert!(conductor.conductor_handle.get_arbitrary_admin_websocket_port().is_none());
        assert!(conductor.conductor_handle.list_app_interfaces().await.unwrap().is_empty());
        assert!(conductor.admin_websocket().await.is_err());
        assert!(conductor.app_websocket("map_holons".into(), AllowedOrigins::Any).await.is_err());
        let caller = initialize_app(&conductor, "map_holons", &happ).await.unwrap();
        let initiator = Arc::new(TrustChannel::new(Arc::new(NativeDanceTransport(caller))));
        let manager = holons_client::init_client_runtime(Some(initiator));
        let runtime = Runtime::new(Arc::new(RuntimeSession::new(manager.clone(), None)));
        let tx = runtime.session().begin_transaction().await.unwrap();
        let context = runtime.session().get_transaction(&tx).unwrap();
        assert!(context.discover_local_holon_space().unwrap().is_none());
        context.enable_bootstrap_provisioning();
        let content_set =
            crate::setup::core_schema_bootstrap::bootstrap_content_set_from_directory(
                &resources.join("core-schema-bootstrap"),
            )
            .unwrap();
        let result = runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context: context.clone(),
                    action: TransactionAction::LoadHolons { content_set },
                }),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap();
        let MapResult::Reference(result) = result else { panic!("Expected loader result") };
        assert_eq!(
            result.property_value("LoadCommitStatus").unwrap(),
            Some("Complete".to_base_value())
        );
        let id = context.get_space_holon().unwrap().unwrap().holon_id().unwrap();
        runtime.session().archive_transaction(&tx).unwrap();
        drop(context);
        drop(runtime);
        drop(manager);
        conductor.shutdown().await.unwrap();
        drop(conductor);

        let conductor =
            HolochainRuntime::launch(vec_to_locked(vec![]), configuration()).await.unwrap();
        let caller = initialize_app(&conductor, "map_holons", &happ).await.unwrap();
        let manager = holons_client::init_client_runtime(Some(Arc::new(TrustChannel::new(
            Arc::new(NativeDanceTransport(caller)),
        ))));
        let context =
            manager.get_transaction_manager().open_public_transaction(manager.clone()).unwrap();
        assert_eq!(context.discover_local_holon_space().unwrap().unwrap().holon_id().unwrap(), id);
        assert!(!context.is_bootstrap_provisioning());
        assert!(conductor.conductor_handle.get_arbitrary_admin_websocket_port().is_none());
        assert!(conductor.conductor_handle.list_app_interfaces().await.unwrap().is_empty());
        drop(context);
        drop(manager);
        conductor.shutdown().await.unwrap();
        drop(conductor);
        std::fs::remove_dir_all(root).unwrap();
    }
}
