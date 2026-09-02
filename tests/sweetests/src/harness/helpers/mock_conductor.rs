use async_trait::async_trait;
use base_types::MapString;
use holochain::conductor::api::error::ConductorApiError;
use holochain::conductor::CellError;
use holochain::core::ribosome::error::RibosomeError;
use holochain::prelude::AgentPubKey;
use holochain::prelude::{
    CoordinatorZomeDef, CoordinatorZomes, DnaDef, DnaHash, DnaWasm, Record, WasmHash, ZomeError,
    ZomeName,
};
use holochain::sweettest::{
    SweetAgents, SweetCell, SweetConductor, SweetConductorConfig, SweetDnaFile,
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

/// Implements envelope transport for the Sweetest mock conductor backend.
///
#[async_trait]
impl DanceEnvelopeTransport for MockConductorConfig {
    async fn initiate_dance_envelope(
        &self,
        envelope: DanceRequestEnvelope,
    ) -> Result<DanceResponseEnvelope, HolonError> {
        let is_load_holons = envelope.request.dance_name.0 == "load_holons";
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
                elapsed_ms = started.elapsed().as_millis(),
                "sweettest transport: load_holons conductor call complete"
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

/// Builds the standard sweettest conductor config with every WAN endpoint redirected to an
/// unroutable local address.
///
/// Sweettests run a single in-process conductor and never peer, so their network dependency is
/// accidental rather than designed. `SweetConductorConfig::standard()` inherits
/// `NetworkConfig::default()`, which hardcodes the public Holochain bootstrap/signal servers and
/// the iroh canary relay — costing per-conductor `net_report` probe timeouts on startup and
/// tripping local outbound firewalls. Pointing all three at `127.0.0.1:1` mirrors the host's
/// dev-mode profile in `launch/config.rs` rather than inventing a second convention.
///
/// TLS schemes (`wss`/`https`) deliberately, not `ws`/`http`: nothing here sets
/// `signalAllowPlainText` or `relayAllowPlainText`, and both transports reject a plaintext URL
/// without the matching flag.
///
/// Only the URLs are touched. `advanced` is left as `standard()` built it — unlike the host's dev
/// mode, it is not cleared, because `standard()` stores its `k2Gossip` interval tuning there and
/// clearing it would silently revert those timings. `target_arc_factor` is likewise left alone:
/// with no peers there is nothing to gossip with, so lowering it would change DHT arc behaviour
/// without removing any traffic.
fn local_only_config() -> SweetConductorConfig {
    SweetConductorConfig::standard().tune_network_config(|network| {
        network.bootstrap_url = url2::url2!("http://127.0.0.1:1");
        network.signal_url = url2::url2!("wss://127.0.0.1:1");
        network.relay_url = url2::url2!("https://127.0.0.1:1");
    })
}

/// Initializes a new Holochain SweetConductor instance for use in integration tests.
///
/// This helper function:
/// - Loads the DNA bundle defined by `DNA_FILEPATH`.
/// - Spawns a SweetConductor whose WAN endpoints are all pointed at unroutable local addresses
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

    let mut conductor = SweetConductor::from_config(local_only_config()).await;
    let holochain_agent = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", holochain_agent.clone(), &[dna.clone()])
        .await
        .unwrap();

    let cell = app.into_cells()[0].clone();
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
        request: DanceRequestWire {
            dance_name: MapString("get_all_holons".into()),
            dance_type: DanceTypeWire::Standalone,
            body: RequestBodyWire::None,
        },
        session: Some(session),
    };

    let response: DanceResponseEnvelope = backend
        .conductor
        .call_fallible(&backend.cell.zome(PRODUCTION_COORDINATOR_ZOME), "dance_adapter", envelope)
        .await
        .expect("the unanchored ordinary dance must reach the production coordinator");
    assert_eq!(
        response.response.status_code,
        ResponseStatusCode::Conflict,
        "the unanchored ordinary dance must be rejected: {:?}",
        response.response
    );
    assert!(
        response
            .response
            .description
            .0
            .contains("ordinary transaction is missing its persisted local HolonSpace"),
        "the rejection must explain the missing persisted LocalHolonSpace: {:?}",
        response.response
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
    let mut conductor = SweetConductor::from_config(local_only_config()).await;
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
    let probe_wasm = DnaWasm::from(probe_wasm_bytes);
    let probe_wasm_hash = WasmHash::with_data(&probe_wasm).await;
    let mut probe_definition = CoordinatorZomeDef::from_hash(probe_wasm_hash.clone());
    // This dependency is enforced by DnaFile::update_coordinators and keeps the probe bound to
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

    // Holochain 0.6.3 mutates the cached DnaFile during an update but does not rebuild the
    // RealRibosome's derived zome-dependency index. Reloading through SweetConductor's updated DNA
    // cache makes the appended zome's enforced integrity dependency available to HDK host calls.
    // This is also the upstream coordinator-update test's persistence path.
    conductor.shutdown().await;
    conductor.startup(true).await;

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
        DnaHash::with_data_sync(&after),
        installed_dna_hash,
        "rehashing the effective updated DNA definition must preserve production DNA identity"
    );

    let _: Vec<Record> = conductor
        .call_fallible(&cell.zome(PRODUCTION_COORDINATOR_ZOME), "get_all_holon_nodes", ())
        .await
        .expect("the production holons coordinator must remain callable after augmentation");

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
    definition
        .wasm_hash(&zome_name)
        .unwrap_or_else(|error| panic!("coordinator {name} is not backed by WASM: {error}"))
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

    /// Pins the local-only intent at the config level, where it is deterministic. The suite-level
    /// evidence (no WAN dialing, no NAT-discovery socket churn) is observational and cannot be
    /// asserted from inside a test, so this guards the thing that produces it: an endpoint left
    /// unassigned would silently inherit a public server from `NetworkConfig::default()`.
    #[test]
    fn every_wan_endpoint_is_local_only() {
        let network = &local_only_config().network;

        for (field, url) in [
            ("bootstrap_url", &network.bootstrap_url),
            ("signal_url", &network.signal_url),
            ("relay_url", &network.relay_url),
        ] {
            assert_eq!(url.host_str(), Some("127.0.0.1"), "{field} is not local-only: {url}");
        }
    }

    /// A plaintext scheme would be rejected by the transports' `validate_config` unless the
    /// matching `*AllowPlainText` flag were set, which `local_only_config()` deliberately does
    /// not set. See the doc comment there.
    #[test]
    fn signal_and_relay_use_tls_schemes() {
        let network = &local_only_config().network;

        assert_eq!(network.signal_url.scheme(), "wss");
        assert_eq!(network.relay_url.scheme(), "https");
    }

    /// `standard()` stores its gossip interval tuning in `advanced`; clearing that field to
    /// suppress WAN config would silently revert those timings for every sweettest.
    #[test]
    fn gossip_tuning_from_standard_config_survives() {
        let advanced = local_only_config()
            .network
            .advanced
            .clone()
            .expect("standard() should have populated advanced with gossip tuning");

        assert!(
            advanced.get("k2Gossip").is_some(),
            "k2Gossip tuning was dropped from advanced: {advanced}"
        );
    }
}
