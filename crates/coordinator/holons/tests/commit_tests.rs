use hdk::prelude::*;
// use std::collections::BTreeMap;

mod shared_test;
use shared_test::*;

use holochain::sweettest::{SweetCell, SweetConductor};

use holons::commit_manager::{CommitError, CommitManager, CommitRequestStatusCode, CommitResponse};
use holons::holon_errors::HolonError;
use holons::holon_node::UpdateHolonNodeInput;
use holons::holon_node::*;
use holons::holon_reference::{HolonReference, LocalHolonReference};
use holons::holon_types::{Holon, HolonState};
use holons::relationship::{RelationshipName, RelationshipTarget};
use shared_types_holon::{BaseValue, MapBoolean, MapInteger, MapString, PropertyName};

#[tokio::test(flavor = "multi_thread")]
async fn test_commit_manager() {
    let (conductor, _agent, cell): (SweetConductor, AgentPubKey, SweetCell) =
        setup_conductor().await;

    let mut holon1 = Holon::new();
    holon1.with_property_value(
        PropertyName(MapString("Property Name A".to_string())),
        BaseValue::StringValue(MapString("String Value A".to_string())),
    );

    let record1: Record = conductor
        .call(
            &cell.zome("holons"),
            "create_holon_node",
            holon1.clone().into_node(),
        )
        .await;

    let fetched_holon1_record: Record = conductor
        .call(
            &cell.zome("holons"),
            "get_holon_node",
            record1.action_address(),
        )
        .await;

    let fetched_holon1: Holon = Holon::try_from_node(fetched_holon1_record).unwrap();
    assert_eq!(holon1.property_map, fetched_holon1.property_map);
    assert_eq!(Some(record1), fetched_holon1.saved_node);
    assert_eq!(HolonState::Fetched, fetched_holon1.state);

    let mut holon2 = Holon::new();
    holon2.with_property_value(
        PropertyName(MapString("Property Name B".to_string())),
        BaseValue::BooleanValue(MapBoolean(true)),
    );
    holon2.add_related_holon(
        RelationshipName(MapString("B->A".to_string())),
        RelationshipTarget::One(HolonReference::Local(LocalHolonReference {
            holon_id: None,
            holon: Some(holon1.clone()),
        })),
    );
    assert_eq!(HolonState::New, holon2.state);

    let mut holon3 = Holon::new();
    holon3.with_property_value(
        PropertyName(MapString("Property Name C - Example Change".to_string())),
        BaseValue::IntegerValue(MapInteger(-1)),
    );
    holon3.with_property_value(
        PropertyName(MapString("Zero Int Val".to_string())),
        BaseValue::IntegerValue(MapInteger(0)),
    );
    holon3.add_related_holon(
        RelationshipName(MapString("C->A".to_string())),
        RelationshipTarget::One(HolonReference::Local(LocalHolonReference {
            holon_id: None,
            holon: Some(holon1.clone()),
        })),
    );
    holon3.add_related_holon(
        RelationshipName(MapString("C->B".to_string())),
        RelationshipTarget::One(HolonReference::Local(LocalHolonReference {
            holon_id: None,
            holon: Some(holon2.clone()),
        })),
    );

    let record3: Record = conductor
        .call(
            &cell.zome("holons"),
            "create_holon_node",
            holon3.clone().into_node(),
        )
        .await;

    let fetched_holon3_record: Record = conductor
        .call(
            &cell.zome("holons"),
            "get_holon_node",
            record3.action_address(),
        )
        .await;

    let mut changed_holon3: Holon = Holon::try_from_node(fetched_holon3_record).unwrap();
    assert_eq!(holon3.property_map, changed_holon3.property_map);
    assert_eq!(Some(record3), changed_holon3.saved_node);
    assert_eq!(HolonState::Fetched, changed_holon3.state);

    changed_holon3.with_property_value(
        PropertyName(MapString("Property Name C - Example Change".to_string())),
        BaseValue::IntegerValue(MapInteger(1)),
    );
    assert_eq!(HolonState::Changed, changed_holon3.state);

    let holon4 = Holon::new();

    println!("Testing...");
    // let holon_map = BTreeMap::from([("A".to_string(), holon1), ("B".to_string(), holon2)]);

    let mut commit_manager = CommitManager::default();

    commit_manager.stage("A - Fetched".to_string(), fetched_holon1);
    commit_manager.stage("B - New".to_string(), holon2);
    commit_manager.stage("C - Changed".to_string(), changed_holon3);
    commit_manager.stage("D - New Empty".to_string(), holon4);

    println!("CommitManager: {:#?}", commit_manager);

    let commit_response: CommitResponse =
        test_helper_commit_manager_commit(&mut commit_manager, conductor, cell).await;

    println!("CommitResponse: {:#?}", commit_response);
    assert_eq!(CommitRequestStatusCode::Success, commit_response.status);
    println!("");
    println!("Updated CommitManager: {:#?}", commit_manager);
    assert_eq!(1, commit_manager.staged_holons.len());
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
                let result = conductor
                    .call_fallible(&cell.zome("holons"), "update_holon_node", input)
                    .await;
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
        if v.state != HolonState::Fetched {
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
