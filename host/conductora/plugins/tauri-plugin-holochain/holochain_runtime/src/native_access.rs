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
                return Err(crate::Error::NativeBoundary(
                    "Persisted DNA differs from packaged DNA".into(),
                ));
            }
            match self
                .request(AdminRequest::GetDnaDefinition(Box::new(cell.cell_id.clone())))
                .await?
            {
                AdminResponse::DnaDefinitionReturned(actual) if actual == *dna.dna_def() => {}
                _ => {
                    return Err(crate::Error::NativeBoundary(
                        "Persisted coordinator differs from package; explicit upgrade is required"
                            .into(),
                    ))
                }
            }
        }
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
