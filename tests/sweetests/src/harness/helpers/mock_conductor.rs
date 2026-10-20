use async_trait::async_trait;
use base_types::MapString;
use holochain::conductor::api::error::ConductorApiError;
use holochain::conductor::{CellError, ConductorHandle};
use holochain::core::ribosome::error::RibosomeError;
use holochain::prelude::AgentPubKey;
use holochain::prelude::{
    CoordinatorZomeDef, CoordinatorZomes, DnaDef, DnaHash, DnaWasm, DnaWasmHashed, WasmHash,
    ZomeDef, ZomeError, ZomeName,
};
use holochain::sweettest::{
    DynSweetRendezvous, SweetAgents, SweetCell, SweetConductor, SweetConductorConfig, SweetDnaFile,
    SweetLocalRendezvous,
};
use holons_boundary::envelopes::{DanceRequestEnvelope, DanceResponseEnvelope};
use holons_boundary::session_state::SessionStateWire;
use holons_boundary::{DanceRequestWire, DanceTypeWire, RequestBodyWire};
use holons_core::core_shared_objects::transactions::TxId;
use holons_core::dances::{DanceInitiator, ResponseStatusCode};
use holons_core::HolonError;
use holons_trust_channel::{DanceEnvelopeTransport, TrustChannel};
use integrity_core_types::{HolonNodeModel, LocalId, PropertyMap};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

const DNA_FILEPATH: &str = "../../happ/workdir/map_holons.dna";
pub const PROBE_WASM_FILEPATH: &str =
    "../../happ/target/wasm32-unknown-unknown/release/holons_test_probes.wasm";
const PRODUCTION_COORDINATOR_ZOME: &str = "holons";
const PROBE_COORDINATOR_ZOME: &str = "holons_test_probes";
const INTEGRITY_ZOME: &str = "holons_integrity";
const PROBE_DISPATCH_FUNCTION: &str = "holon_storage_author_create_for_test";
// This helper supplies a fresh unanchored ordinary session, so the id only needs to be
// well-formed; it carries no identity or ordering meaning across calls.
const UNANCHORED_ORDINARY_TX_ID: &str = "1";

#[derive(Debug)]
pub struct MockConductorConfig {
    pub conductor: SweetConductor,
    pub agent: AgentPubKey,
    pub cell: SweetCell,
}

impl MockConductorConfig {
    /// Awaits conductor shutdown instead of leaving it to `SweetConductor::drop`, which only
    /// spawns a detached shutdown task, and which never runs at all while the backend is leaked.
    ///
    /// Drop the `Runtime` and everything derived from it first. The backend can still be leaked
    /// by a reference cycle: cached relationship collections hold `SmartReference`s, whose
    /// `SpaceReadHandle` keeps the owning `HolonSpaceManager` (and so this backend) alive. In that
    /// case the conductor is shut down through its shared handle and the backend is deliberately
    /// leaked, so `SweetConductor::drop` can never attempt a second shutdown.
    pub async fn shutdown(self: Arc<Self>) {
        match Arc::try_unwrap(self) {
            Ok(MockConductorConfig { mut conductor, .. }) => conductor.shutdown().await,
            Err(shared) => {
                let handle: ConductorHandle = (**shared.conductor).clone();
                handle
                    .shutdown()
                    .await
                    .expect("conductor shutdown task failed")
                    .expect("conductor shutdown reported an error");
                std::mem::forget(shared);
            }
        }
    }
}

/// Implements envelope transport for the Sweetest mock conductor backend.
///
#[async_trait]
impl DanceEnvelopeTransport for MockConductorConfig {
    async fn initiate_dance_envelope(
        &self,
        envelope: DanceRequestEnvelope,
    ) -> Result<DanceResponseEnvelope, HolonError> {
        let dance = envelope.request.transport_label().to_owned();
        let is_load_holons = dance == "load_holons";
        let started = Instant::now();
        let result = self
            .conductor
            .call_fallible::<DanceRequestEnvelope, DanceResponseEnvelope>(
                &self.cell.zome("holons"),
                "dance_adapter",
                envelope,
            )
            .await;

        if is_load_holons {
            info!(
                "[PERF-688] sweettest_transport: conductor_call_ms={}",
                started.elapsed().as_millis(),
            );
        }

        if dance == "commit" {
            info!(
                "[commit-transport] conductor_call_us={} success={}",
                started.elapsed().as_micros(),
                result.is_ok()
            );
        }

        match result {
            Ok(response_envelope) => Ok(response_envelope),
            Err(error) => Err(HolonError::ConductorError(format!(
                "SweetConductor dance call failed: {:?}",
                error
            ))),
        }
    }
}

/// Uses a live loopback rendezvous server while preserving standard gossip tuning.
///
/// Holochain waits for the relay to assign an address before publishing local agent info.
/// An unreachable relay therefore prevents app installation even for a single-agent test.
/// Sweettest enables plaintext relay support for this local test infrastructure.
///
/// Each conductor is an isolated single-agent node, so DHT publish and gossip have no peers.
/// Left on, they re-scan every authored op (about 38k after the Core Schema bootstrap) every
/// few seconds and contend with the test's own zome calls for the database.
///
/// Gossip is also what grows an agent's storage arc, so with it disabled the agent stays a
/// non-authority and every read tries the network first. Setup therefore calls
/// `declare_full_storage_arcs` after installing the app, as Holochain's own tests do.
fn local_only_config(rendezvous: &DynSweetRendezvous) -> SweetConductorConfig {
    SweetConductorConfig::standard()
        .tune_network_config(|network| {
            network.disable_publish = true;
            network.disable_gossip = true;
        })
        .apply_rendezvous(rendezvous)
}

/// Keeps the local rendezvous alive with the conductor, including across probe restarts.
async fn create_local_conductor() -> SweetConductor {
    let rendezvous = SweetLocalRendezvous::new().await;
    SweetConductor::from_config_rendezvous(local_only_config(&rendezvous), rendezvous).await
}

/// Initializes a new Holochain SweetConductor instance for use in integration tests.
///
/// This helper function:
/// - Loads the DNA bundle defined by `DNA_FILEPATH`.
/// - Spawns a SweetConductor whose bootstrap and relay endpoints use a live loopback server
///   (see [`local_only_config()`]).
/// - Creates a single test agent and installs the DNA.
/// - Extracts the initialized [`SweetCell`], [`AgentPubKey`], and [`SweetConductor`] into a
///   [`MockConductorConfig`] backend suitable for use by higher-level test utilities.
///
/// # Returns
/// An [`Arc<MockConductorConfig>`] containing a fully initialized test conductor, agent,
/// and cell — ready to be wrapped in a [`DanceInitiator`] implementation such as
/// [`TrustChannel`].
///
/// # Panics
/// This function will panic if:
/// - The DNA bundle cannot be read or parsed from `DNA_FILEPATH`.
/// - The conductor fails to start or install the DNA.
/// - The SweetTest environment cannot allocate an agent or cell.
///
/// # Examples
/// ```ignore
/// let backend = setup_test_conductor().await;
/// let initiator = TrustChannel::new(backend);
/// ```
pub async fn setup_test_conductor() -> Arc<MockConductorConfig> {
    info!("Current working directory: {:?}", std::env::current_dir().unwrap());

    let dna = SweetDnaFile::from_bundle(std::path::Path::new(DNA_FILEPATH)).await.unwrap();

    let mut conductor = create_local_conductor().await;
    let holochain_agent = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", holochain_agent.clone(), &[dna.clone()])
        .await
        .unwrap();

    let cell = app.into_cells()[0].clone();
    conductor.declare_full_storage_arcs(cell.dna_hash()).await;
    let agent_hash = holochain_agent.into_inner();
    let agent = AgentPubKey::from_raw_39(agent_hash);

    Arc::new(MockConductorConfig { conductor, agent, cell })
}

/// Proves that a production dance cannot run with an unanchored ordinary session.
///
/// Initial CoreSchemaSpace provisioning is host-owned. Guest sessions may use the bootstrap
/// marker only when the host explicitly constructs that provisioning transaction.
pub async fn assert_unanchored_ordinary_session_is_rejected(backend: &MockConductorConfig) {
    let mut session = SessionStateWire::default();
    session.set_tx_id(
        TxId::from_str(UNANCHORED_ORDINARY_TX_ID)
            .expect("the fixed test transaction id must be valid"),
    );
    let envelope = DanceRequestEnvelope {
        request: holons_boundary::envelopes::DanceEnvelopeRequest::Legacy(DanceRequestWire {
            dance_name: MapString("get_all_holons".into()),
            dance_type: DanceTypeWire::Standalone,
            body: RequestBodyWire::None,
        }),
        session: Some(session),
    };

    let response: DanceResponseEnvelope = backend
        .conductor
        .call_fallible(&backend.cell.zome(PRODUCTION_COORDINATOR_ZOME), "dance_adapter", envelope)
        .await
        .expect("the unanchored ordinary dance must reach the production coordinator");
    let holons_boundary::envelopes::DanceEnvelopeResponse::Legacy(response) = response.response
    else {
        panic!("expected legacy response")
    };
    assert_eq!(
        response.status_code,
        ResponseStatusCode::Conflict,
        "the unanchored ordinary dance must be rejected: {:?}",
        response
    );
    assert!(
        response
            .description
            .0
            .contains("ordinary transaction is missing its persisted local HolonSpace"),
        "the rejection must explain the missing persisted LocalHolonSpace: {:?}",
        response
    );
}

/// Installs the production DNA and then appends the loose test-probe coordinator in an isolated
/// conductor while proving that the production coordinator and Integrity boundary are unchanged.
///
/// Coordinator updates mutate the DNA registered within a conductor. A backend returned by this
/// helper must therefore never be pooled with, or reused by, production-only assertions. Each
/// probe-dependent test must create its own backend through this helper.
///
/// The probe WASM is loaded from [`PROBE_WASM_FILEPATH`] rather than inferred from a Cargo target
/// directory. The `sweetest` build ordering is responsible for freshly producing that exact loose
/// artifact before any test calls this helper.
///
/// # Panics
///
/// Panics if the production DNA or probe WASM cannot be read, conductor installation or the
/// coordinator update fails, the probe is callable before augmentation, or any coordinator/DNA
/// invariance assertion fails.
pub async fn setup_probe_enabled_conductor() -> Arc<MockConductorConfig> {
    info!("Current working directory: {:?}", std::env::current_dir().unwrap());

    let dna = SweetDnaFile::from_bundle(std::path::Path::new(DNA_FILEPATH)).await.unwrap();
    let mut conductor = create_local_conductor().await;
    let holochain_agent = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("probe-enabled-app", holochain_agent.clone(), &[dna])
        .await
        .unwrap();

    let cell = app.into_cells()[0].clone();
    let cell_id = cell.cell_id().clone();
    let installed_dna_hash = cell.dna_hash().clone();

    assert_probe_zome_unavailable(&conductor, &cell).await;

    let before = conductor
        .get_dna_def(&cell_id)
        .expect("the installed production cell must have an active DNA definition");
    let before_coordinator_names = coordinator_names(&before);
    assert_eq!(
        before_coordinator_names,
        BTreeSet::from([PRODUCTION_COORDINATOR_ZOME.to_string()]),
        "the installed production DNA must start with exactly the holons coordinator"
    );
    let production_wasm_hash_before = coordinator_wasm_hash(&before, PRODUCTION_COORDINATOR_ZOME);
    let integrity_zomes_before = before.integrity_zomes.clone();

    let probe_wasm_bytes = std::fs::read(PROBE_WASM_FILEPATH).unwrap_or_else(|error| {
        panic!("failed to read loose probe WASM at {PROBE_WASM_FILEPATH}: {error}")
    });
    let probe_wasm = DnaWasmHashed::from_content(DnaWasm::from(probe_wasm_bytes)).await;
    let probe_wasm_hash = probe_wasm.hash.clone();
    let mut probe_definition = CoordinatorZomeDef::from_hash(probe_wasm_hash.clone());
    // This dependency is enforced by DnaDef::replace_coordinators and keeps the probe bound to
    // the production Integrity zome it is intended to exercise.
    probe_definition.set_dependency(INTEGRITY_ZOME);

    // Supplying only the new name is essential: including a second `holons` definition would
    // replace the active production coordinator instead of merely augmenting the test conductor.
    let coordinator_update: CoordinatorZomes =
        vec![(PROBE_COORDINATOR_ZOME.into(), probe_definition)];
    conductor
        .update_coordinators(cell_id.clone(), coordinator_update, vec![probe_wasm])
        .await
        .expect("the isolated conductor must accept the probe-only coordinator update");

    // Reload the persisted DNA definition and WASM so the assertions also verify that the
    // appended coordinator and its integrity dependency survive a conductor restart.
    conductor.shutdown().await;
    conductor.startup().await;
    // The restart rejoins the agent with an empty storage arc; claim the full arc again.
    conductor.declare_full_storage_arcs(&installed_dna_hash).await;

    let after = conductor
        .get_dna_def(&cell_id)
        .expect("the updated cell must retain an active DNA definition");
    assert_eq!(
        coordinator_names(&after),
        BTreeSet::from([
            PRODUCTION_COORDINATOR_ZOME.to_string(),
            PROBE_COORDINATOR_ZOME.to_string(),
        ]),
        "the update must append only holons_test_probes"
    );
    assert_eq!(
        coordinator_wasm_hash(&after, PRODUCTION_COORDINATOR_ZOME),
        production_wasm_hash_before,
        "the probe update must not replace the active production coordinator"
    );
    assert_eq!(
        coordinator_wasm_hash(&after, PROBE_COORDINATOR_ZOME),
        probe_wasm_hash,
        "the active probe definition must reference the freshly read loose WASM"
    );
    assert_eq!(
        after.integrity_zomes, integrity_zomes_before,
        "coordinator augmentation must not change Integrity definitions"
    );
    assert_eq!(
        DnaHash::with_data_sync(&after.content),
        installed_dna_hash,
        "rehashing the effective updated DNA definition must preserve production DNA identity"
    );

    let agent_hash = holochain_agent.into_inner();
    let agent = AgentPubKey::from_raw_39(agent_hash);
    Arc::new(MockConductorConfig { conductor, agent, cell })
}

/// Proves dispatch rejects the probe at zome resolution when only the production DNA is installed.
pub async fn assert_probe_zome_unavailable(conductor: &SweetConductor, cell: &SweetCell) {
    let result = conductor
        .call_fallible::<_, LocalId>(
            &cell.zome(PROBE_COORDINATOR_ZOME),
            PROBE_DISPATCH_FUNCTION,
            valid_probe_holon_node(),
        )
        .await;

    // SweetConductor dispatch crosses the cell and ribosome boundaries before zome-name
    // resolution, so preserve those stable wrappers while requiring the precise inner cause.
    assert!(
        matches!(
            result,
            Err(ConductorApiError::CellError(CellError::RibosomeError(
                RibosomeError::ConductorApiError(ref inner)
            ))) if matches!(inner.as_ref(), ConductorApiError::ZomeError(ZomeError::ZomeNotFound(_)))
        ),
        "a production-only cell must reject the unregistered probe zome, got {result:?}"
    );
}

/// Proves dispatch executes a registered probe function after administrative augmentation.
pub async fn assert_probe_zome_dispatchable(conductor: &SweetConductor, cell: &SweetCell) {
    let _: LocalId = conductor
        .call_fallible(
            &cell.zome(PROBE_COORDINATOR_ZOME),
            PROBE_DISPATCH_FUNCTION,
            valid_probe_holon_node(),
        )
        .await
        .expect("the augmented cell must execute the registered probe function");
}

fn coordinator_names(dna_def: &DnaDef) -> BTreeSet<String> {
    dna_def.coordinator_zomes.iter().map(|(name, _)| name.to_string()).collect()
}

fn coordinator_wasm_hash(dna_def: &DnaDef, name: &str) -> WasmHash {
    let zome_name = ZomeName::from(name);
    let definition = dna_def
        .coordinator_zomes
        .iter()
        .find_map(|(candidate, definition)| (candidate == &zome_name).then_some(definition))
        .unwrap_or_else(|| panic!("coordinator {name} is absent from the active DNA definition"));
    match definition.as_any_zome_def() {
        ZomeDef::Wasm(wasm) => wasm.wasm_hash.clone(),
        ZomeDef::Inline(_) => panic!("coordinator {name} is not backed by WASM"),
    }
}

fn valid_probe_holon_node() -> HolonNodeModel {
    // An empty property map is valid under current PVL rules and keeps this assertion focused on
    // coordinator dispatch rather than on any domain value chosen solely for the harness.
    HolonNodeModel::new(PropertyMap::new())
}

/// Constructs a test [`DanceInitiator`] implementation backed by a mock Holochain conductor.
///
/// This function builds upon [`setup_test_conductor()`] to:
/// 1. Spawn a SweetConductor-based [`MockConductorConfig`] backend.
/// 2. Wrap the backend in a [`TrustChannel`], which adds envelope and
///    session_state-state coordination for DANCE invocations.
/// 3. Return the wrapped instance as a trait object suitable for dependency injection
///    into a [`HolonSpaceManager`].
///
/// # Returns
/// An [`Arc<dyn DanceInitiator + Send + Sync>`] representing the test
/// transport layer for client-to-conductor DANCE interactions.
///
/// # Use in Tests
/// Typically used by `init_test_context()` to populate the `HolonSpaceManager`
/// with a functioning DANCE initiator:
///
/// ```ignore
/// let dance_initiator = create_test_dance_initiator().await;
/// let space_manager = HolonSpaceManager::new_with_managers(
///     Some(dance_initiator),
///     holon_service,
///     None,
///     ServiceRoutingPolicy::Combined,
/// );
/// ```
///
/// # Panics
/// Propagates any panic from [`setup_test_conductor()`] if the test environment
/// fails to initialize.
///
/// # See Also
/// - [`setup_test_conductor()`] — underlying conductor setup
/// - [`TrustChannel`] — envelope-aware DANCE transport implementation
pub async fn create_test_dance_initiator() -> Arc<dyn DanceInitiator + Send + Sync> {
    let backend = setup_test_conductor().await;
    Arc::new(TrustChannel::new(backend)) as Arc<dyn DanceInitiator + Send + Sync>
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both endpoints must resolve to the live loopback server, preserving standard tuning.
    #[tokio::test]
    async fn rendezvous_endpoints_are_local_and_preserve_gossip_tuning() {
        let rendezvous = SweetLocalRendezvous::new().await;
        let config = local_only_config(&rendezvous);
        let network = &config.network;

        for (field, url) in
            [("bootstrap_url", &network.bootstrap_url), ("relay_url", &network.relay_url)]
        {
            assert_eq!(url.host_str(), Some("127.0.0.1"), "{field} is not local-only: {url}");
            let port = url.port().expect("local rendezvous must allocate a port");
            tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .expect("rendezvous endpoint must be listening");
        }
        assert_eq!(network.bootstrap_url, url2::url2!("{}", rendezvous.bootstrap_addr()));
        assert_eq!(network.relay_url, url2::url2!("{}", rendezvous.relay_addr()));
        assert_eq!(network.advanced, SweetConductorConfig::standard().network.advanced);
        assert!(network.disable_publish, "isolated conductors must not publish");
        assert!(network.disable_gossip, "isolated conductors must not gossip");
    }
}
