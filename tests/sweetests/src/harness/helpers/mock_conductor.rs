use async_trait::async_trait;
use holochain::prelude::AgentPubKey;
use holochain::sweettest::{
    SweetAgents, SweetCell, SweetConductor, SweetConductorConfig, SweetDnaFile,
};
use holons_boundary::envelopes::{DanceRequestEnvelope, DanceResponseEnvelope};
use holons_core::dances::DanceInitiator;
use holons_core::HolonError;
use holons_trust_channel::{DanceEnvelopeTransport, TrustChannel};
use std::sync::Arc;
use tracing::info;

const DNA_FILEPATH: &str = "../../happ/workdir/map_holons.dna";

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
        let result = self
            .conductor
            .call_fallible::<DanceRequestEnvelope, DanceResponseEnvelope>(
                &self.cell.zome("holons"),
                "dance_adapter",
                envelope,
            )
            .await;

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
