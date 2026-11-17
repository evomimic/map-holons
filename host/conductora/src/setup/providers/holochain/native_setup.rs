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
    let upgrade = std::env::args_os().any(|arg| arg == "--upgrade-coordinators");
    anyhow::ensure!(
        !upgrade || cfg!(all(feature = "local-constitution", debug_assertions)),
        "Explicit local coordinator upgrade requires the packaged local-constitution build"
    );
    let caller = initialize_app_with_upgrade(
        runtime,
        &config.app_id,
        &handle.path().resource_dir()?.join("resources/map-holons.happ"),
        upgrade,
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
#[cfg(test)]
async fn initialize_app(
    runtime: &tauri_plugin_holochain::HolochainRuntime,
    app_id: &str,
    happ_path: &std::path::Path,
) -> anyhow::Result<NativeZomeCaller> {
    initialize_app_with_upgrade(runtime, app_id, happ_path, false).await
}

async fn initialize_app_with_upgrade(
    runtime: &tauri_plugin_holochain::HolochainRuntime,
    app_id: &str,
    happ_path: &std::path::Path,
    upgrade: bool,
) -> anyhow::Result<NativeZomeCaller> {
    let admin = runtime.native_administration();
    let apps = admin.list_apps().await?;
    anyhow::ensure!(apps.len() <= 1, "Packaged startup supports one installed application");
    let existing = apps.into_iter().next();
    anyhow::ensure!(
        !upgrade || existing.is_some(),
        "Coordinator upgrade requires an existing application; it cannot provision an installation"
    );
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
    if upgrade {
        let started = std::time::Instant::now();
        let changed = admin.upgrade_packaged_coordinators(&app, happ_path).await?;
        tracing::info!(
            changed,
            elapsed_ms = started.elapsed().as_millis(),
            "[UPGRADE] Explicit coordinator-only upgrade completed"
        );
    } else {
        admin.verify_packaged_app(&app, happ_path).await?;
    }
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
    use base_types::ToBaseValue;
    use holons_core::{descriptors::Descriptor, reference_layer::ReadableHolon};
    use map_commands_runtime::{
        personal_constitution::{
            inspect_target, ConstitutionOutcome, PersonalConstitution, PersonalName,
            SovereigntyObservation,
        },
        Runtime, RuntimeSession,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use type_names::{
        AgentSpaceHolonTypeName, AgentSpacePropertyTypeName, AgentSpaceRelationshipTypeName,
        CoreRelationshipTypeName,
    };

    #[derive(Debug)]
    struct CountingTransport {
        inner: NativeDanceTransport,
        loads: Arc<AtomicUsize>,
        discoveries: Arc<AtomicUsize>,
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
            if request.request.transport_label() == "discover_local_holon_space" {
                self.discoveries.fetch_add(1, Ordering::SeqCst);
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
    async fn native_only_keyless_personal_constitution_preserves_space() {
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
        let discoveries = Arc::new(AtomicUsize::new(0));
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
            discoveries: discoveries.clone(),
        })));
        let manager = holons_client::init_client_runtime(Some(initiator));
        let runtime = Runtime::new(Arc::new(RuntimeSession::new(manager.clone(), None)));
        let resource_root = resources.parent().unwrap();
        assert_eq!(
            prepare_core_schema(&runtime, resource_root, None).await.unwrap(),
            CoreSchemaStartup::Provisioned
        );
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert_eq!(discoveries.load(Ordering::SeqCst), 1);
        // Open does not inspect or replay import files. A missing dependency is
        // resolved by its consumer, not by revalidating the saved foundation.
        prepare_space_imports(
            &runtime,
            &root.join("no-import-files-on-open"),
            CoreSchemaStartup::OpenedExisting,
            None,
        )
        .await
        .unwrap();
        assert!(inspect_target(&runtime).await.is_err());
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        prepare_space_imports(&runtime, resource_root, CoreSchemaStartup::Provisioned, None)
            .await
            .unwrap();
        assert_eq!(loads.load(Ordering::SeqCst), 3);
        let target = inspect_target(&runtime).await.unwrap();
        assert_eq!(target.sovereignty, SovereigntyObservation::NoAssociationObserved);
        let id = target.space;
        assert!(
            map_commands_runtime::application_launch::select_application_launch(
                &runtime,
                map_commands_runtime::application_launch::LaunchRequirement::AcquiredSpace,
                |_| Ok(())
            )
            .await
            .is_err(),
            "unclaimed spaces cannot launch acquired Navigator"
        );
        assert_eq!(discoveries.load(Ordering::SeqCst), 1, "status reuses the adopted Space");
        let tx = runtime.session().begin_transaction().await.unwrap();
        let context = runtime.session().get_transaction(&tx).unwrap();
        let descriptor_id = context
            .lookup()
            .get_saved_holon_by_key(
                &AgentSpaceHolonTypeName::PersonAgent
                    .descriptor_key(AgentSpaceHolonTypeName::BioSocialAgent),
            )
            .unwrap()
            .holon_id();
        runtime.session().archive_transaction(&tx).unwrap();
        drop(context);
        let baseline = loads.load(Ordering::SeqCst);
        // Create in the same runtime that provisioned the foundation, matching Welcome.
        // This test-owned conductor has no production admission or Keychain claim.
        let admission = PersonalConstitution::default();
        assert!(admission.available());
        let (outcome, competing) = tokio::join!(
            admission.submit(
                &runtime,
                PersonalName {
                    last_name: "Example".into(),
                    first_name: Some("Alex".into()),
                    middle_name: None,
                },
            ),
            admission.submit(
                &runtime,
                PersonalName { last_name: "Competing".into(), first_name: None, middle_name: None },
            ),
        );
        let outcome = outcome.unwrap();
        assert!(competing.is_err(), "a concurrent Welcome request must not stage another Agent");
        assert_eq!(outcome, ConstitutionOutcome::Complete);
        // This runtime observed an empty inverse before Commit. Welcome must
        // now observe saved membership instead of reusing that cached absence.
        let after_creation = inspect_target(&runtime).await.unwrap();
        assert_eq!(after_creation.space, id);
        assert_eq!(
            discoveries.load(Ordering::SeqCst),
            1,
            "fresh membership must not rediscover Space identity"
        );
        assert_eq!(after_creation.sovereignty, SovereigntyObservation::AssociationPresent);
        let verify_tx = runtime.session().begin_transaction().await.unwrap();
        let verify_context = runtime.session().get_transaction(&verify_tx).unwrap();
        let space = verify_context.discover_local_holon_space().unwrap().unwrap();
        assert_eq!(space.holon_id().unwrap(), id);
        let owners = space
            .related_holons_with_hint(
                AgentSpaceRelationshipTypeName::IsInteriorOf,
                holons_core::RelationshipReadHint::RequireFresh,
            )
            .unwrap();
        let person = {
            let owners = owners.read().unwrap();
            assert_eq!(owners.get_members().len(), 1);
            owners.get_members()[0].clone()
        };
        let created_id = person.holon_id().unwrap();
        assert_eq!(person.key().unwrap(), None);
        assert_eq!(
            person.property_value(AgentSpacePropertyTypeName::LastName).unwrap(),
            Some("Example".to_base_value())
        );
        assert_eq!(
            person.property_value(AgentSpacePropertyTypeName::FirstName).unwrap(),
            Some("Alex".to_base_value())
        );
        assert_eq!(person.property_value(AgentSpacePropertyTypeName::MiddleName).unwrap(), None);
        for name in [
            type_names::ToRelationshipName::to_relationship_name(
                AgentSpaceRelationshipTypeName::HasInterior,
            ),
            type_names::ToRelationshipName::to_relationship_name(CoreRelationshipTypeName::OwnedBy),
        ] {
            let related = person
                .related_holons_with_hint(name, holons_core::RelationshipReadHint::RequireFresh)
                .unwrap();
            assert_eq!(related.read().unwrap().get_members().len(), 1);
            assert_eq!(related.read().unwrap().get_members()[0].holon_id().unwrap(), id);
        }
        assert_eq!(person.holon_descriptor().unwrap().holon().holon_id().unwrap(), descriptor_id);
        runtime.session().archive_transaction(&verify_tx).unwrap();
        let launch = map_commands_runtime::application_launch::select_application_launch(
            &runtime,
            map_commands_runtime::application_launch::LaunchRequirement::AcquiredSpace,
            |_| Ok(()),
        )
        .await
        .unwrap();
        assert_eq!(launch.space, id);
        assert_eq!(launch.agent, Some(created_id.clone()));
        assert!(launch.home.is_some());
        assert_eq!(loads.load(Ordering::SeqCst), baseline, "Navigator launch does not import");
        assert_eq!(discoveries.load(Ordering::SeqCst), 1, "Navigator reuses adopted Space");

        assert!(admission
            .submit(
                &runtime,
                PersonalName { last_name: "Another".into(), first_name: None, middle_name: None },
            )
            .await
            .is_err());
        assert_eq!(loads.load(Ordering::SeqCst), baseline);
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
        let conductor = HolochainRuntime::launch(protected.clone(), configuration()).await.unwrap();
        let caller = initialize_app(&conductor, "map_holons", &happ).await.unwrap();
        assert_eq!(
            conductor.native_administration().list_apps().await.unwrap()[0].agent_pub_key,
            author
        );
        let manager = holons_client::init_client_runtime(Some(Arc::new(TrustChannel::new(
            Arc::new(CountingTransport {
                inner: NativeDanceTransport(caller),
                loads: loads.clone(),
                discoveries: discoveries.clone(),
            }),
        ))));
        let runtime = Runtime::new(Arc::new(RuntimeSession::new(manager.clone(), None)));
        let baseline = loads.load(Ordering::SeqCst);
        assert_eq!(
            prepare_core_schema(&runtime, resource_root, None).await.unwrap(),
            CoreSchemaStartup::OpenedExisting
        );
        prepare_space_imports(&runtime, resource_root, CoreSchemaStartup::OpenedExisting, None)
            .await
            .unwrap();
        let reopened = inspect_target(&runtime).await.unwrap();
        assert_eq!(reopened.space, id);
        assert_eq!(
            discoveries.load(Ordering::SeqCst),
            2,
            "one guest discovery per cold host runtime"
        );
        assert_eq!(reopened.sovereignty, SovereigntyObservation::AssociationPresent);
        assert!(!crate::welcome_lifecycle::blockers(&reopened).is_empty());
        let tx = runtime.session().begin_transaction().await.unwrap();
        let context = runtime.session().get_transaction(&tx).unwrap();
        assert_eq!(
            context
                .lookup()
                .get_saved_holon_by_key(
                    &AgentSpaceHolonTypeName::PersonAgent
                        .descriptor_key(AgentSpaceHolonTypeName::BioSocialAgent)
                )
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
        assert_eq!(inspect_target(&runtime).await.unwrap().space, id);
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

        // Reconstruct host caches and the conductor after the PersonAgent commit.
        let conductor = HolochainRuntime::launch(protected, configuration()).await.unwrap();
        let caller = initialize_app(&conductor, "map_holons", &happ).await.unwrap();
        let manager = holons_client::init_client_runtime(Some(Arc::new(TrustChannel::new(
            Arc::new(CountingTransport {
                inner: NativeDanceTransport(caller),
                loads: loads.clone(),
                discoveries: discoveries.clone(),
            }),
        ))));
        let reopened_runtime = Runtime::new(Arc::new(RuntimeSession::new(manager.clone(), None)));
        assert_eq!(
            prepare_core_schema(&reopened_runtime, resource_root, None).await.unwrap(),
            CoreSchemaStartup::OpenedExisting
        );
        prepare_space_imports(
            &reopened_runtime,
            resource_root,
            CoreSchemaStartup::OpenedExisting,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            conductor.native_administration().list_apps().await.unwrap()[0].agent_pub_key,
            author
        );
        let observed = inspect_target(&reopened_runtime).await.unwrap();
        assert_eq!(observed.space, id);
        assert_eq!(discoveries.load(Ordering::SeqCst), 3, "warm status adds no guest discovery");
        assert_eq!(observed.sovereignty, SovereigntyObservation::AssociationPresent);
        // A new backend admission object after restart must also refuse constitution.
        assert!(PersonalConstitution::default()
            .submit(
                &reopened_runtime,
                PersonalName { last_name: "Repeat".into(), first_name: None, middle_name: None },
            )
            .await
            .is_err());
        let tx = reopened_runtime.session().begin_transaction().await.unwrap();
        let context = reopened_runtime.session().get_transaction(&tx).unwrap();
        let retained_space = context.discover_local_holon_space().unwrap().unwrap();
        assert_eq!(retained_space.holon_id().unwrap(), id);
        let association = retained_space
            .related_holons_with_hint(
                AgentSpaceRelationshipTypeName::IsInteriorOf,
                holons_core::RelationshipReadHint::RequireFresh,
            )
            .unwrap();
        let retained_person = association.read().unwrap().get_members()[0].clone();
        assert_eq!(retained_person.holon_id().unwrap(), created_id);
        assert_eq!(retained_person.key().unwrap(), None);
        assert_eq!(
            retained_person.property_value(AgentSpacePropertyTypeName::LastName).unwrap(),
            Some("Example".to_base_value())
        );
        assert_eq!(loads.load(Ordering::SeqCst), baseline);
        reopened_runtime.session().archive_transaction(&tx).unwrap();
        conductor.shutdown().await.unwrap();
        drop(conductor);
        std::fs::remove_dir_all(root).unwrap();
    }
}
