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
    use crate::setup::{
        core_schema_bootstrap::{prepare_core_schema, CoreSchemaStartup},
        space_imports::prepare_space_imports,
    };
    use map_commands_runtime::{Runtime, RuntimeSession};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct CountingTransport {
        inner: NativeDanceTransport,
        loads: Arc<AtomicUsize>,
    }
    #[async_trait::async_trait]
    impl DanceEnvelopeTransport for CountingTransport {
        async fn initiate_dance_envelope(
            &self,
            request: DanceRequestEnvelope,
        ) -> Result<DanceResponseEnvelope, HolonError> {
            if request.request.transport_label() == "load_holons" {
                self.loads.fetch_add(1, Ordering::SeqCst);
            }
            self.inner.initiate_dance_envelope(request).await
        }
    }
    use tauri_plugin_holochain::{
        vec_to_locked, AllowedOrigins, HolochainRuntime, HolochainRuntimeConfig,
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
            let packaged: serde_json::Value =
                serde_json::from_str(include_str!("../../../config/storage.packaged.json"))
                    .unwrap();
            let config: HolochainConfig =
                serde_json::from_value(packaged["storage_providers"]["holochain_packaged"].clone())
                    .unwrap();
            let mut network =
                crate::setup::providers::holochain::plugins::network_config_from_storage_config(
                    &config,
                );
            network.bootstrap_url = url2::Url2::parse("http://127.0.0.1:1");
            network.relay_url = url2::Url2::parse("https://127.0.0.1:1");
            HolochainRuntimeConfig::new(root.clone(), network).backend_only(true)
        };
        let loads = Arc::new(AtomicUsize::new(0));
        let conductor =
            HolochainRuntime::launch(vec_to_locked(vec![]), configuration()).await.unwrap();
        assert!(conductor.conductor_handle.get_arbitrary_admin_websocket_port().is_none());
        assert!(conductor.conductor_handle.list_app_interfaces().await.unwrap().is_empty());
        assert!(conductor.admin_websocket().await.is_err());
        assert!(conductor.app_websocket("map_holons".into(), AllowedOrigins::Any).await.is_err());
        assert!(HolochainRuntime::launch(vec_to_locked(vec![]), configuration()).await.is_err());
        let caller = initialize_app(&conductor, "map_holons", &happ).await.unwrap();
        let author =
            conductor.native_administration().list_apps().await.unwrap()[0].agent_pub_key.clone();
        let initiator = Arc::new(TrustChannel::new(Arc::new(CountingTransport {
            inner: NativeDanceTransport(caller),
            loads: loads.clone(),
        })));
        let manager = holons_client::init_client_runtime(Some(initiator));
        let runtime = Runtime::new(Arc::new(RuntimeSession::new(manager.clone(), None)));
        let resource_root = resources.parent().unwrap();
        assert_eq!(
            prepare_core_schema(&runtime, resource_root, None).await.unwrap(),
            CoreSchemaStartup::Provisioned
        );
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        // Interrupted post-Core provisioning is refused on Open, not repaired by import replay.
        assert!(prepare_space_imports(&runtime, resource_root, CoreSchemaStartup::OpenedExisting)
            .await
            .is_err());
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        prepare_space_imports(&runtime, resource_root, CoreSchemaStartup::Provisioned)
            .await
            .unwrap();
        assert_eq!(loads.load(Ordering::SeqCst), 3);
        let target = crate::constitution_admission::inspect_target(&runtime).await.unwrap();
        let id = target.space;
        let tx = runtime.session().begin_transaction().await.unwrap();
        let context = runtime.session().get_transaction(&tx).unwrap();
        let descriptor_id = context
            .lookup()
            .get_saved_holon_by_key(&"PersonAgent.BioSocialAgent".into())
            .unwrap()
            .holon_id();
        runtime.session().archive_transaction(&tx).unwrap();
        drop(context);
        drop(runtime);
        drop(manager);
        conductor.shutdown().await.unwrap();
        drop(conductor);

        // Only this test-owned installation is changed. Production cutover's
        // authenticated roll-forward/fault cases are tested in credential_custody.
        std::fs::write(root.join(".credential-cutover"), b"interrupted fixture").unwrap();
        assert!(HolochainRuntime::launch(vec_to_locked(vec![]), configuration()).await.is_err());
        std::fs::remove_file(root.join(".credential-cutover")).unwrap();
        let protected = vec_to_locked(vec![8; 32]);
        let wrappers = tauri_plugin_holochain::credential_custody::prepare_passphrase_transition(
            &root,
            vec_to_locked(vec![]),
            protected.clone(),
        )
        .await
        .unwrap();
        std::fs::write(
            root.join("keystore/lair-keystore-config.yaml"),
            wrappers.lair_configuration,
        )
        .unwrap();
        std::fs::write(root.join("conductor/databases/db.key"), wrappers.database_key_file)
            .unwrap();
        assert!(HolochainRuntime::launch(vec_to_locked(vec![]), configuration()).await.is_err());
        let conductor = HolochainRuntime::launch(protected, configuration()).await.unwrap();
        let caller = initialize_app(&conductor, "map_holons", &happ).await.unwrap();
        assert_eq!(
            conductor.native_administration().list_apps().await.unwrap()[0].agent_pub_key,
            author
        );
        let manager =
            holons_client::init_client_runtime(Some(Arc::new(TrustChannel::new(Arc::new(
                CountingTransport { inner: NativeDanceTransport(caller), loads: loads.clone() },
            )))));
        let runtime = Runtime::new(Arc::new(RuntimeSession::new(manager.clone(), None)));
        let baseline = loads.load(Ordering::SeqCst);
        assert_eq!(
            prepare_core_schema(&runtime, resource_root, None).await.unwrap(),
            CoreSchemaStartup::OpenedExisting
        );
        prepare_space_imports(&runtime, resource_root, CoreSchemaStartup::OpenedExisting)
            .await
            .unwrap();
        let reopened = crate::constitution_admission::inspect_target(&runtime).await.unwrap();
        assert_eq!(reopened.space, id);
        assert_eq!(
            reopened.sovereignty,
            crate::constitution_admission::SovereigntyObservation::NoAssociationObserved
        );
        assert!(!crate::constitution_admission::blockers(&reopened).is_empty());
        let tx = runtime.session().begin_transaction().await.unwrap();
        let context = runtime.session().get_transaction(&tx).unwrap();
        assert_eq!(
            context
                .lookup()
                .get_saved_holon_by_key(&"PersonAgent.BioSocialAgent".into())
                .unwrap()
                .holon_id(),
            descriptor_id
        );
        assert!(!context.is_bootstrap_provisioning());
        runtime.session().archive_transaction(&tx).unwrap();
        drop(context);
        // A package/version mismatch must not trigger a replacement Space or reload.
        let incompatible = root.join("incompatible-resources");
        let destination = incompatible.join("resources/core-schema-bootstrap");
        std::fs::create_dir_all(&destination).unwrap();
        let mut manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(resources.join("core-schema-bootstrap/manifest.json")).unwrap(),
        )
        .unwrap();
        manifest["core_schema_key"] = "Unsupported Core Schema version".into();
        std::fs::write(destination.join("manifest.json"), serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        assert!(prepare_core_schema(&runtime, &incompatible, None).await.is_err());
        assert_eq!(
            crate::constitution_admission::inspect_target(&runtime).await.unwrap().space,
            id
        );
        assert_eq!(
            loads.load(Ordering::SeqCst),
            baseline,
            "Open, missing dependencies and incompatible state must never replay imports"
        );
        drop(runtime);
        assert!(conductor.conductor_handle.get_arbitrary_admin_websocket_port().is_none());
        assert!(conductor.conductor_handle.list_app_interfaces().await.unwrap().is_empty());
        drop(manager);
        conductor.shutdown().await.unwrap();
        drop(conductor);
        std::fs::remove_dir_all(root).unwrap();
    }
}
