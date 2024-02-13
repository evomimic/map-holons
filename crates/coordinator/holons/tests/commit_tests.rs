use hdk::prelude::*;
// use std::collections::BTreeMap;

mod shared_test;
use shared_test::*;

use holochain::sweettest::{SweetCell, SweetConductor};

use holons::commit_manager::{CommitError, CommitManager, CommitRequestStatusCode, CommitResponse};
use holons::holon_errors::HolonError;
use holons::holon_node::UpdateHolonNodeInput;
use holons::holon_node::*;
use holons::holon_types::{Holon, HolonState};
use shared_types_holon::MapString;

#[tokio::test(flavor = "multi_thread")]
async fn test_commit_manager() {
    let (conductor, _agent, cell): (SweetConductor, AgentPubKey, SweetCell) =
        setup_conductor().await;

    let holon1 = Holon::new();
    let holon2 = Holon::new();

    println!("Testing...");
    // let holon_map = BTreeMap::from([("A".to_string(), holon1), ("B".to_string(), holon2)]);

    let mut commit_manager = CommitManager::default();

    commit_manager.stage("A".to_string(), holon1);
    commit_manager.stage("B".to_string(), holon2);

    println!("CommitManager: {:#?}", commit_manager);

    let commit_response: CommitResponse =
        test_helper_commit_manager_commit(&mut commit_manager, conductor, cell).await;

    println!("CommitResponse: {:#?}", commit_response);

    // let fetched_holons: Vec<Holon> = conductor
    //     .call(&cell.zome("holons"), "get_all_holons", ())
    //     .await;
    // assert_eq!(0, fetched_holons.len());

    println!("");
}

// HELPERS //

pub async fn test_helper_holon_commit(
    holon: Holon,
    conductor: &SweetConductor,
    cell: &SweetCell,
) -> Result<Holon, HolonError> {
    let mut holon = holon.clone();
    match holon.state {
        HolonState::New => {
            // Create a new HolonNode from this Holon and request it be created
            let result = conductor
                .call_fallible(
                    &cell.zome("holons"),
                    "create_holon_node",
                    holon.clone().into_node(),
                )
                .await;
            match result {
                Ok(record) => {
                    holon.saved_node = Some(record);
                    holon.state = HolonState::Fetched;

                    Ok(holon)
                }
                Err(error) => Err(HolonError::from(error)),
            }
        }
        HolonState::Fetched => {
            // Holon hasn't been changed since it was fetched
            return Ok(holon);
        }
        HolonState::Changed => {
            if let Some(node) = holon.saved_node.clone() {
                let input = UpdateHolonNodeInput {
                    // TEMP solution for original hash is to keep it the same //
                    original_holon_node_hash: node.action_address().clone(), // TODO: find way to populate this correctly
                    previous_holon_node_hash: node.action_address().clone(),
                    updated_holon_node: holon.clone().into_node(),
                };
                let result = update_holon_node(input);
                match result {
                    Ok(record) => {
                        holon.saved_node = Some(record);

                        Ok(holon)
                    }
                    Err(error) => Err(HolonError::from(error)),
                }
            } else {
                Err(HolonError::HolonNotFound(
                    "Must have a saved node in order to update".to_string(),
                ))
            }
        }
    }
}

pub async fn test_helper_commit_manager_commit(
    commit_manager: &mut CommitManager,
    conductor: SweetConductor,
    cell: SweetCell,
) -> CommitResponse {
    let mut errors: Vec<CommitError> = Vec::new();
    for (k, v) in commit_manager.clone().staged_holons.iter() {
        let result = test_helper_holon_commit(v.clone(), &conductor, &cell).await;
        match result {
            Ok(_) => {
                commit_manager.staged_holons.remove(k.into());
            }
            Err(e) => {
                let commit_error = CommitError {
                    holon_key: MapString(k.to_string()),
                    error_code: e,
                    // description: MapString("".to_string()),
                };
                errors.push(commit_error);
            }
        }
    }
    let error_count = errors.len();
    if errors.is_empty() {
        let commit_response = CommitResponse {
            status: CommitRequestStatusCode::Success,
            description: MapString("All holons successfully committed".to_string()),
            errors: None,
        };
        return commit_response;
    }
    let commit_response = CommitResponse {
        status: CommitRequestStatusCode::Success,
        description: MapString(format!("Error committing {:?} holons", error_count)),
        errors: Some(errors),
    };
    commit_response
}
