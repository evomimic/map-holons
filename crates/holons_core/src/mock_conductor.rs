// use std::sync::Arc;

// use async_trait::async_trait;
use holochain::prelude::AgentPubKey;
use holochain::sweettest::{SweetAgents, SweetCell, SweetConductor, SweetDnaFile};

use crate::dances::{ConductorDanceCaller, DanceRequest, DanceResponse};

// use holons_prelude::prelude::*;

const DNA_FILEPATH: &str = "../../workdir/map_holons.dna";

#[derive(Debug)]
pub struct MockConductorConfig {
    pub conductor: SweetConductor,
    pub agent: AgentPubKey,
    pub cell: SweetCell,
}

/// Implements `DanceCaller` for the Sweetest mock conductor.
///
/// This allows `MockConductorConfig` to be used inside `DanceCallService`.
#[async_trait::async_trait(?Send)]
impl ConductorDanceCaller for MockConductorConfig {
    async fn conductor_dance_call(&self, request: DanceRequest) -> DanceResponse {
        let res = self.conductor.call(&self.cell.zome("holons"), "dance", request).await;
        DanceResponse::from(res)
    }
}

/// MOCK CONDUCTOR

pub async fn setup_conductor() -> MockConductorConfig {
    println!("Current working directory: {:?}", std::env::current_dir().unwrap());
    let dna = SweetDnaFile::from_bundle(std::path::Path::new(&DNA_FILEPATH)).await.unwrap();

    let mut conductor = SweetConductor::from_standard_config().await;

    let holochain_agent = SweetAgents::one(conductor.keystore()).await;

    let app = conductor
        .setup_app_for_agent("app", holochain_agent.clone(), &[dna.clone()])
        .await
        .unwrap();

    let cell = app.into_cells()[0].clone();

    let agent_hash = holochain_agent.into_inner();
    let agent = AgentPubKey::from_raw_39(agent_hash);

    MockConductorConfig { conductor, agent, cell }
}
