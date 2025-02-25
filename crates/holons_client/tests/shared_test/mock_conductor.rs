use holochain::prelude::AgentPubKey;
use holochain::sweettest::{SweetAgents, SweetCell, SweetConductor, SweetDnaFile};
use holons_client::ConductorDanceCaller;
use holons_core::dances::{DanceRequest, DanceResponse};

const DNA_FILEPATH: &str = "../../../workdir/map_holons.dna";

#[derive(Debug)]
pub struct MockConductorConfig {
    pub conductor: SweetConductor,
    pub agent: AgentPubKey,
    pub cell: SweetCell,
}

/// Implements `DanceCaller` for the Sweetest mock conductor.
///
/// This allows `MockConductorConfig` to be used inside `DanceCallService`.
impl ConductorDanceCaller for MockConductorConfig {
    fn conductor_dance_call(&self, request: DanceRequest) -> DanceResponse {
        futures::executor::block_on(async {
            self.conductor.call(&self.cell.zome("dances"), "dance", request).await
        })
    }
}

/// MOCK CONDUCTOR

pub async fn setup_conductor() -> MockConductorConfig {
    let dna = SweetDnaFile::from_bundle(std::path::Path::new(&DNA_FILEPATH)).await.unwrap();

    let mut conductor = SweetConductor::from_standard_config().await;

    let holochain_agent = SweetAgents::one(conductor.keystore()).await;

    let app = conductor
        .setup_app_for_agent("app", holochain_agent.clone(), &[dna.clone()])
        .await
        .unwrap();

    let cell = app.into_cells()[0].clone();

    let agent_hash = holochain_agent.into_inner();
    let agent = AgentPubKey::from_raw_39(agent_hash).unwrap();

    MockConductorConfig { conductor, agent, cell }
}
