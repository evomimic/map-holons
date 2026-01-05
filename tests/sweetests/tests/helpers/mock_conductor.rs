use async_trait::async_trait;
use holochain::prelude::AgentPubKey;
use holochain::sweettest::*;
use holochain::sweettest::{SweetAgents, SweetCell, SweetConductor, SweetDnaFile};
use holons_core::dances::{DanceInitiator, DanceRequest, DanceResponse};
use holons_core::{HolonError, HolonsContextBehavior};
use holons_prelude::prelude::*;
use holons_trust_channel::TrustChannel;
use std::sync::Arc;
use tracing::info;

const DNA_FILEPATH: &str = "../../happ/workdir/map_holons.dna";

#[derive(Debug)]
pub struct MockConductorConfig {
    pub conductor: SweetConductor,
    pub agent: AgentPubKey,
    pub cell: SweetCell,
}

/// Implements the [`DanceInitiator`] trait for the [`MockConductorConfig`],
/// enabling simulated Holochain conductor calls within the Sweetest test environment.
///
/// This implementation bridges the `DanceInitiator` interface to the mock
/// conductor used in integration tests. It sends a `DanceRequest` into the
/// "holons" zome of the test conductor, awaits the zome call result, and
/// converts the outcome into a standardized [`DanceResponse`].
///
/// Unlike production implementations, this test variant does **not** return a
/// `Result`. Any underlying conductor or zome-level error is encapsulated as a
/// [`DanceResponse`] with an appropriate [`ResponseStatusCode`], ensuring
/// consistent handling of all outcomes within test logic.
///
/// # Arguments
///
/// * `_context` — The current [`HolonsContextBehavior`] context (unused in
///   this mock implementation, but included for trait compatibility).
/// * `request` — The [`DanceRequest`] to execute within the test conductor.
///
/// # Returns
///
/// A [`DanceResponse`] representing either a successful zome call or a
/// structured failure response, depending on the outcome of the conductor call.
///
/// # Example
///
/// ```rust
/// let request = DanceRequest::new("SomeDance", payload);
/// let ctx: &(dyn HolonsContextBehavior + Send + Sync) = context;
/// let response = run_future_synchronously(initiator.initiate_dance(ctx, request));
/// assert_eq!(response.status_code(), ResponseStatusCode::Ok);
/// ```
#[async_trait]
impl DanceInitiator for MockConductorConfig {
    async fn initiate_dance(
        &self,
        _context: &(dyn HolonsContextBehavior + Send + Sync),
        request: DanceRequest,
    ) -> DanceResponse {
        let res = self.conductor.call(&self.cell.zome("holons"), "dance", request).await;

        DanceResponse::from(res)
    }
}

/// Initializes a new Holochain SweetConductor instance for use in integration tests.
///
/// This helper function:
/// - Loads the DNA bundle defined by `DNA_FILEPATH`.
/// - Spawns a standard SweetConductor with default configuration.
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

    let mut conductor = SweetConductor::from_standard_config().await;
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
///    session-state coordination for DANCE invocations.
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
///     nursery,
///     transient_manager,
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
