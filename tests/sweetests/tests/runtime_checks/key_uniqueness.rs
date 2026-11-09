//! Reimport rejection against the suite's established Book/Person schema.
use core_types::CommitValidationViolationKind;
use holons_core::core_shared_objects::holon::StagedState;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::build_book_person_inverse_content_set;
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime};

pub async fn reimporting_domain_creates_is_a_key_uniqueness_rejection(runtime: &Runtime) {
    let MapResult::TransactionCreated { tx_id } = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("transaction")
    };
    let context = runtime.session().get_transaction(&tx_id).unwrap();
    let result = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: context.clone(),
                action: TransactionAction::LoadHolons {
                    content_set: build_book_person_inverse_content_set().unwrap(),
                },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap();
    let MapResult::Reference(HolonReference::Transient(response)) = result else {
        panic!("loader response")
    };
    assert_eq!(
        response.property_value(CorePropertyTypeName::LoadCommitStatus).unwrap(),
        Some("Rejected".to_base_value())
    );
    assert_eq!(
        response.property_value(CorePropertyTypeName::HolonsCommitted).unwrap(),
        Some(MapInteger(0).to_base_value())
    );
    assert_eq!(
        response.property_value(CorePropertyTypeName::ErrorCount).unwrap(),
        Some(MapInteger(0).to_base_value())
    );
    let candidates = context.staged_references().unwrap();
    assert!(!candidates.is_empty());
    assert!(candidates
        .iter()
        .all(|c| !matches!(c.staged_state().unwrap(), StagedState::Committed(_))));
    assert!(candidates.iter().any(|c| c.validation_findings().unwrap().iter().any(|v| matches!(&v.kind, CommitValidationViolationKind::RuleViolation { code } if code == "DS-KEY-006"))));
    runtime.session().dispose_transaction(&context.tx_id()).await.unwrap();
}
