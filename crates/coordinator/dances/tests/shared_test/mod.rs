// #![allow(warnings)]

pub mod fixtures;
pub use fixtures::*;

pub mod data_types;
pub mod test_abandon_staged_changes;
pub mod test_add_related_holon;
pub mod test_commit;
pub mod test_ensure_database_count;
pub mod test_load_core_schema;
pub mod test_match_db_content;
pub mod test_print_database;
pub mod test_query_relationships;
pub mod test_remove_related_holon;
pub mod test_stage_new_from_clone;
pub mod test_stage_new_holon;
pub mod test_stage_new_version;
pub mod test_with_properties_command;

use data_types::DanceTestState;
use hdk::prelude::*;
use holochain::sweettest::{SweetAgents, SweetCell, SweetConductor, SweetDnaFile};
use holons::holon_reference::HolonGettable;
use holons::{
    context::HolonsContext, holon::Holon, holon_error::HolonError, holon_reference::HolonReference,
    relationship::RelationshipName,
};
use shared_types_holon::{HolonId, MapString};

const DNA_FILEPATH: &str = "../../../workdir/map_holons.dna";

/// MOCK CONDUCTOR

pub async fn setup_conductor() -> (SweetConductor, AgentPubKey, SweetCell) {
    let dna = SweetDnaFile::from_bundle(std::path::Path::new(&DNA_FILEPATH))
        .await
        .unwrap();

    // let dna_path = std::env::current_dir().unwrap().join(DNA_FILEPATH);
    // println!("{}", dna_path.to_string_lossy());
    // let dna = SweetDnaFile::from_bundle(&dna_path).await.unwrap();

    let mut conductor = SweetConductor::from_standard_config().await;

    let holo_core_agent = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", holo_core_agent.clone(), &[dna.clone()])
        .await
        .unwrap();

    let cell = app.into_cells()[0].clone();

    let agent_hash = holo_core_agent.into_inner();
    let agent = AgentPubKey::from_raw_39(agent_hash).unwrap();

    (conductor, agent, cell)
}
