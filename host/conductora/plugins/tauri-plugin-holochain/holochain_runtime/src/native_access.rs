//! Native access to the existing conductor APIs, without external administration or app sockets.
use holochain::conductor::{api::AdminInterfaceApi, ConductorHandle};
use holochain_client::InstallAppPayload;
use holochain_conductor_api::{AdminRequest, AdminResponse, AppInfo};
use holochain_types::prelude::{AgentPubKey, CellId};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

/// Narrow native administration used by first-cell startup. Never registered as IPC.
#[derive(Clone)]
pub struct NativeAdministration {
    api: AdminInterfaceApi,
}

impl NativeAdministration {
    pub(crate) fn new(handle: ConductorHandle) -> Self {
        Self { api: AdminInterfaceApi::new(handle) }
    }

    async fn request(&self, request: AdminRequest) -> crate::Result<AdminResponse> {
        let response = self
            .api
            .handle_request(Ok(request))
            .await
            .map_err(|e| crate::Error::NativeBoundary(e.to_string()))?;
        match response {
            AdminResponse::Error(error) => Err(crate::Error::NativeBoundary(format!("{error:?}"))),
            response => Ok(response),
        }
    }

    pub async fn list_apps(&self) -> crate::Result<Vec<AppInfo>> {
        match self.request(AdminRequest::ListApps { status_filter: None }).await? {
            AdminResponse::AppsListed(apps) => Ok(apps),
            _ => Err(crate::Error::NativeBoundary("Unexpected list-apps response".into())),
        }
    }

    pub async fn generate_agent_pub_key(&self) -> crate::Result<AgentPubKey> {
        match self.request(AdminRequest::GenerateAgentPubKey).await? {
            AdminResponse::AgentPubKeyGenerated(key) => Ok(key),
            _ => Err(crate::Error::NativeBoundary("Unexpected key-generation response".into())),
        }
    }

    pub async fn install_app(&self, payload: InstallAppPayload) -> crate::Result<AppInfo> {
        match self.request(AdminRequest::InstallApp(Box::new(payload))).await? {
            AdminResponse::AppInstalled(app) => Ok(app),
            _ => Err(crate::Error::NativeBoundary("Unexpected installation response".into())),
        }
    }

    /// Reject persisted executable state that differs from this package; do not silently upgrade.
    pub async fn verify_packaged_app(
        &self,
        app: &AppInfo,
        path: &std::path::Path,
    ) -> crate::Result<()> {
        let bundle =
            holochain_types::prelude::AppBundle::unpack(std::io::Cursor::new(std::fs::read(path)?))
                .map_err(crate::UpdateHappError::AppBundleError)?;
        let expected = crate::happs::update::resolve_dna_files(bundle).await?;
        if app.cell_info.len() != 1 || expected.len() != 1 {
            return Err(crate::Error::NativeBoundary("Expected one packaged role".into()));
        }
        for (role, dna) in expected {
            let cells = app
                .cell_info
                .get(&role)
                .ok_or_else(|| crate::Error::NativeBoundary("Packaged role is absent".into()))?;
            if cells.len() != 1 {
                return Err(crate::Error::NativeBoundary("Expected one packaged cell".into()));
            }
            let holochain_conductor_api::CellInfo::Provisioned(cell) = &cells[0] else {
                return Err(crate::Error::NativeBoundary("Unsupported cell kind".into()));
            };
            if cell.cell_id.dna_hash() != dna.dna_hash() {
                return Err(crate::Error::IncompatibleEnvironment(format!(
                    "Persisted DNA {} differs from packaged DNA {}",
                    cell.cell_id.dna_hash(),
                    dna.dna_hash()
                )));
            }
            match self
                .request(AdminRequest::GetDnaDefinition(Box::new(cell.cell_id.clone())))
                .await?
            {
                AdminResponse::DnaDefinitionReturned(actual) if actual == *dna.dna_def() => {}
                _ => {
                    return Err(crate::Error::IncompatibleEnvironment(
                        "Persisted coordinator differs from package; explicit upgrade is required"
                            .into(),
                    ))
                }
            }
        }
        self.ensure_no_remote_grants(app).await
    }

    async fn ensure_no_remote_grants(&self, app: &AppInfo) -> crate::Result<()> {
        // The initial native-only profile has no remote executor. Refuse saved grants
        // rather than leave a P2P route around the disabled client operations.
        match self
            .request(AdminRequest::ListCapabilityGrants {
                installed_app_id: app.installed_app_id.clone(),
                include_revoked: false,
            })
            .await?
        {
            AdminResponse::CapabilityGrantsInfo(info)
                if info.0.iter().all(|(_, grants)| grants.is_empty()) => {}
            _ => {
                return Err(crate::Error::NativeBoundary(
                    "Remote capability grants are outside the initial native-only profile".into(),
                ))
            }
        }
        Ok(())
    }

    /// Explicit code-only update for the existing packaged cell. Never exposed over IPC.
    /// Reject integrity, role, coordinator-set and dependency changes before any mutation.
    /// Reapplying an already installed package is a no-op; no authored data is recreated.
    pub async fn upgrade_packaged_coordinators(
        &self,
        app: &AppInfo,
        path: &std::path::Path,
    ) -> crate::Result<bool> {
        use holochain_types::prelude::{
            AppBundle, CoordinatorBundle, CoordinatorManifest, UpdateCoordinatorsPayload,
            ZomeDependency, ZomeManifest,
        };
        let bundle = AppBundle::unpack(std::io::Cursor::new(std::fs::read(path)?))
            .map_err(crate::UpdateHappError::AppBundleError)?;
        let expected = crate::happs::update::resolve_dna_files(bundle).await?;
        if app.cell_info.len() != 1 || expected.len() != 1 {
            return Err(crate::Error::NativeBoundary("Expected one packaged role".into()));
        }
        let (role, dna) = expected.into_iter().next().unwrap();
        let cells = app
            .cell_info
            .get(&role)
            .ok_or_else(|| crate::Error::NativeBoundary("Packaged role is absent".into()))?;
        let [holochain_conductor_api::CellInfo::Provisioned(cell)] = cells.as_slice() else {
            return Err(crate::Error::NativeBoundary(
                "Expected one provisioned packaged cell".into(),
            ));
        };
        if cell.cell_id.dna_hash() != dna.dna_hash()
            || cell.cell_id.agent_pubkey() != &app.agent_pub_key
        {
            return Err(crate::Error::NativeBoundary(
                "Coordinator-only upgrade cannot change DNA or author identity".into(),
            ));
        }
        let actual = match self
            .request(AdminRequest::GetDnaDefinition(Box::new(cell.cell_id.clone())))
            .await?
        {
            AdminResponse::DnaDefinitionReturned(actual) => actual,
            _ => {
                return Err(crate::Error::NativeBoundary("Missing persisted DNA definition".into()))
            }
        };
        let target = dna.dna_def();
        if actual.name != target.name
            || actual.modifiers != target.modifiers
            || actual.integrity_zomes != target.integrity_zomes
            || actual.coordinator_zomes.len() != target.coordinator_zomes.len()
            || actual.coordinator_zomes.iter().zip(&target.coordinator_zomes).any(
                |((old_name, old), (new_name, new))| {
                    old_name != new_name
                        || old.clone().erase_type().dependencies()
                            != new.clone().erase_type().dependencies()
                },
            )
        {
            return Err(crate::Error::NativeBoundary("Coordinator-only upgrade cannot change integrity, coordinator names or dependencies".into()));
        }
        self.ensure_no_remote_grants(app).await?;
        let mut zomes = Vec::new();
        let mut resources = Vec::new();
        for ((_, old), (name, new)) in
            actual.coordinator_zomes.iter().zip(&target.coordinator_zomes)
        {
            if old.zome_hash() == new.zome_hash() {
                continue;
            }
            zomes.push(ZomeManifest {
                name: name.clone(),
                hash: None,
                path: name.0.to_string(),
                dependencies: Some(
                    new.clone()
                        .erase_type()
                        .dependencies()
                        .iter()
                        .cloned()
                        .map(|name| ZomeDependency { name })
                        .collect(),
                ),
            });
            let wasm = dna.get_wasm_for_zome(name).map_err(crate::UpdateHappError::DnaError)?;
            resources.push((name.0.to_string(), wasm.clone().code().to_vec().into()));
        }
        let changed = !zomes.is_empty();
        if changed {
            let source: CoordinatorBundle =
                mr_bundle::Bundle::new(CoordinatorManifest { zomes }, resources)
                    .map_err(crate::UpdateHappError::MrBundleError)?
                    .into();
            match self
                .request(AdminRequest::UpdateCoordinators(Box::new(UpdateCoordinatorsPayload {
                    cell_id: cell.cell_id.clone(),
                    source: holochain_types::prelude::CoordinatorSource::Bundle(Box::new(source)),
                })))
                .await?
            {
                AdminResponse::CoordinatorsUpdated => {}
                _ => {
                    return Err(crate::Error::NativeBoundary(
                        "Unexpected coordinator-update response".into(),
                    ))
                }
            }
        }
        self.verify_packaged_app(app, path).await?;
        let saved = self
            .list_apps()
            .await?
            .into_iter()
            .find(|saved| saved.installed_app_id == app.installed_app_id)
            .ok_or_else(|| {
                crate::Error::NativeBoundary("Application missing after coordinator upgrade".into())
            })?;
        if saved.agent_pub_key != app.agent_pub_key || saved.cell_info != app.cell_info {
            return Err(crate::Error::NativeBoundary(
                "Application identity changed during coordinator upgrade".into(),
            ));
        }
        tracing::info!(
            changed,
            "[UPGRADE] Verified packaged coordinator code; existing cell and author retained"
        );
        Ok(changed)
    }

    pub async fn enable_app(&self, installed_app_id: String) -> crate::Result<AppInfo> {
        match self.request(AdminRequest::EnableApp { installed_app_id }).await? {
            AdminResponse::AppEnabled(app) => Ok(app),
            _ => Err(crate::Error::NativeBoundary("Unexpected enable-app response".into())),
        }
    }
}

/// A host-held caller fixed to one cell and the existing Dance ingress.
/// No raw persistence function or caller-selected zome/function is exposed.
#[derive(Clone)]
pub struct NativeZomeCaller {
    handle: ConductorHandle,
    cell: CellId,
}

impl Debug for NativeZomeCaller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeZomeCaller").field("cell", &self.cell).finish_non_exhaustive()
    }
}

impl NativeZomeCaller {
    pub(crate) fn new(handle: ConductorHandle, cell: CellId) -> Self {
        Self { handle, cell }
    }

    pub async fn call<I: Serialize + Debug, O: DeserializeOwned + Debug>(
        &self,
        input: I,
    ) -> crate::Result<O> {
        self.handle
            .easy_call_zome(
                self.cell.agent_pubkey(),
                None,
                self.cell.clone(),
                "holons",
                "dance",
                input,
            )
            .await
            .map_err(|e| crate::Error::NativeBoundary(e.to_string()))
    }
}

/// Inspect persisted listener configuration before Holochain can restore any sockets.
/// Reuse the conductor's own state loader; this does not alter AgentSpace identity or content.
pub(crate) async fn ensure_no_saved_interfaces(
    config: &holochain::conductor::config::ConductorConfig,
    passphrase: lair_keystore_api::types::SharedLockedArray,
) -> crate::Result<()> {
    let spaces =
        holochain::conductor::space::Spaces::new(std::sync::Arc::new(config.clone()), passphrase)
            .await?;
    if !spaces.get_state().await?.app_interfaces.is_empty() {
        return Err(crate::Error::NativeBoundary("Existing conductor has saved app listeners; explicit interface reconciliation is required before native-only startup".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Change only a test-owned WASM custom section: valid code with a different hash.
    async fn package_variant(source: &std::path::Path, destination: &std::path::Path, zome: &str) {
        use holochain_types::prelude::{AppBundle, DnaBundle};
        let app = AppBundle::unpack(std::io::Cursor::new(std::fs::read(source).unwrap()))
            .unwrap()
            .into_inner();
        let (dna_key, dna_bytes) = app.get_all_resources().into_iter().next().unwrap();
        let dna = DnaBundle::unpack(dna_bytes.as_ref()).unwrap();
        let file = dna.clone().to_dna_file().await.unwrap().0;
        let old = file.get_wasm_for_zome(&zome.into()).unwrap().clone().code();
        let resources = dna
            .get_all_resources()
            .into_iter()
            .map(|(key, bytes)| {
                let mut bytes = bytes.as_ref().to_vec();
                if bytes.as_slice() == old.as_ref() {
                    bytes.extend_from_slice(&[0, 2, 1, b'x']);
                }
                (key.clone(), bytes.into())
            })
            .collect::<Vec<_>>();
        let changed =
            mr_bundle::Bundle::new(dna.manifest().clone(), resources).unwrap().pack().unwrap();
        let resources = app
            .get_all_resources()
            .into_iter()
            .map(|(key, bytes)| {
                (key.clone(), if key == dna_key { changed.to_vec().into() } else { bytes.clone() })
            })
            .collect::<Vec<_>>();
        let package =
            mr_bundle::Bundle::new(app.manifest().clone(), resources).unwrap().pack().unwrap();
        std::fs::write(destination, package).unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn explicit_coordinator_upgrade_preserves_identity_and_survives_restart() {
        use holochain_types::prelude::AppBundleSource;
        let root = std::env::temp_dir().join(format!(
            "map-coordinator-upgrade-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let package = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../happ/workdir/map-holons.happ");
        let changed = root.join("changed.happ");
        let incompatible = root.join("incompatible.happ");
        package_variant(&package, &changed, "holons").await;
        package_variant(&package, &incompatible, "holons_integrity").await;
        let configuration = || {
            let mut network = crate::NetworkConfig::default();
            network.bootstrap_url = url2::Url2::parse("http://127.0.0.1:1");
            network.relay_url = url2::Url2::parse("https://127.0.0.1:1");
            network.target_arc_factor = 0;
            crate::HolochainRuntimeConfig::new(root.join("store"), network).backend_only(true)
        };
        let runtime =
            crate::HolochainRuntime::launch(crate::vec_to_locked(vec![]), configuration())
                .await
                .unwrap();
        let admin = runtime.native_administration();
        let author = admin.generate_agent_pub_key().await.unwrap();
        admin
            .install_app(InstallAppPayload {
                agent_key: Some(author.clone()),
                roles_settings: None,
                network_seed: None,
                source: AppBundleSource::Path(package.clone()),
                installed_app_id: Some("map_holons".into()),
                ignore_genesis_failure: false,
                restore_from_dht: false,
            })
            .await
            .unwrap();
        let app = admin.enable_app("map_holons".into()).await.unwrap();
        admin.verify_packaged_app(&app, &package).await.unwrap();
        assert!(matches!(admin.verify_packaged_app(&app, &incompatible).await,
            Err(crate::Error::IncompatibleEnvironment(message)) if message.contains("DNA")));
        assert!(admin.upgrade_packaged_coordinators(&app, &incompatible).await.is_err());
        admin.verify_packaged_app(&app, &package).await.unwrap();
        assert!(admin.upgrade_packaged_coordinators(&app, &changed).await.unwrap());
        assert!(matches!(admin.verify_packaged_app(&app, &package).await,
            Err(crate::Error::IncompatibleEnvironment(message)) if message.contains("coordinator")));
        assert!(!admin.upgrade_packaged_coordinators(&app, &changed).await.unwrap());
        let after = admin.list_apps().await.unwrap().remove(0);
        assert_eq!(after.agent_pub_key, author);
        assert_eq!(after.cell_info, app.cell_info);
        assert!(runtime.conductor_handle.get_arbitrary_admin_websocket_port().is_none());
        assert!(runtime.conductor_handle.list_app_interfaces().await.unwrap().is_empty());
        drop(admin);
        runtime.shutdown().await.unwrap();
        drop(runtime);
        let runtime =
            crate::HolochainRuntime::launch(crate::vec_to_locked(vec![]), configuration())
                .await
                .unwrap();
        let admin = runtime.native_administration();
        let reopened = admin.list_apps().await.unwrap().remove(0);
        assert_eq!(reopened.agent_pub_key, author);
        assert_eq!(reopened.cell_info, app.cell_info);
        admin.verify_packaged_app(&reopened, &changed).await.unwrap();
        drop(admin);
        runtime.shutdown().await.unwrap();
        drop(runtime);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn saved_external_interface_is_refused_before_restart() {
        let root = std::env::temp_dir().join(format!(
            "map-interface-rejection-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let configuration = || {
            let mut network = crate::NetworkConfig::default();
            network.bootstrap_url = url2::Url2::parse("http://127.0.0.1:1");
            network.relay_url = url2::Url2::parse("https://127.0.0.1:1");
            network.target_arc_factor = 0;
            crate::HolochainRuntimeConfig::new(root.clone(), network).backend_only(true)
        };
        let runtime =
            crate::HolochainRuntime::launch(crate::vec_to_locked(vec![]), configuration())
                .await
                .unwrap();
        assert!(runtime.conductor_handle.get_arbitrary_admin_websocket_port().is_none());
        // Trusted test-only access seeds the precise legacy listener state.
        runtime
            .native_administration()
            .request(AdminRequest::AttachAppInterface {
                port: Some(0),
                danger_bind_addr: None,
                allowed_origins: holochain_types::websocket::AllowedOrigins::Any,
                installed_app_id: None,
            })
            .await
            .unwrap();
        runtime.shutdown().await.unwrap();
        drop(runtime);
        let result =
            crate::HolochainRuntime::launch(crate::vec_to_locked(vec![]), configuration()).await;
        match result {
            Err(crate::Error::NativeBoundary(message)) => {
                assert!(message.contains("saved app listeners"))
            }
            Err(error) => panic!("Unexpected error: {error}"),
            Ok(runtime) => {
                runtime.shutdown().await.unwrap();
                panic!("Restored a saved listener");
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
