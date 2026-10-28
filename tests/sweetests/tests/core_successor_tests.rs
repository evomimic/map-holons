//! Public Commit and loader coverage for exact Core predecessor and successor contracts.
// Shared step executors include suite entrypoints unused by this isolated case.
#[allow(dead_code)]
mod execution_steps;
#[allow(dead_code)]
mod fixture_cases {
    pub mod commit_schema_fixture;
    pub mod described_instance_helpers;
    pub mod load_holons_internal_fixture;
    pub mod schema_authoring_helpers;
}

use execution_steps::dance_test_runner::run_dance_test_case;
use fixture_cases::commit_schema_fixture::{
    commit_core_successor_fixture, loader_core_successor_fixture,
};
use holons_core::core_shared_objects::holon::StagedState;
use holons_core::RelationshipReadHint;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::init_test_runtime;
use holons_test::{DancesTestCase, TestExecutionState};
use map_commands_contract::{MapCommand, MapResult, SpaceCommand};

/// Each test needs a fresh runtime: publishing or rekeying a Core lineage head changes
/// later key-based lookups, while existing descriptors retain exact predecessor references.
/// Sharing this Space with other fixtures would make their results depend on test order.
async fn run_core_fixture(case: DancesTestCase) -> TestExecutionState {
    let mut bootstrap = DancesTestCase::default();
    let (runtime, tx_id) = init_test_runtime(&mut bootstrap).await;
    let mut state = TestExecutionState::new(
        runtime,
        tx_id.clone(),
        case.test_session_state.get_transient_holons().clone(),
        case.test_session_state.fixture_head_index().clone(),
    );
    state.activate_transaction(tx_id).unwrap();
    run_dance_test_case(case, &mut state, &mut false, &mut false).await;
    state
}

#[tokio::test(flavor = "multi_thread")]
async fn rekeyed_core_successor_commits_after_independent_rejection() {
    let state = run_core_fixture(commit_core_successor_fixture().unwrap()).await;
    let context = state.context();
    let candidates = context.staged_references().unwrap();
    let update = candidates
        .iter()
        .find(|candidate| candidate.versioned_source_id().unwrap().is_some())
        .expect("Core successor survives the rejected attempt and commits on retry");
    let StagedState::Committed(successor_id) = update.staged_state().unwrap() else {
        panic!("rekeyed Core successor must persist");
    };
    let source_id = HolonId::Local(update.versioned_source_id().unwrap().unwrap());
    let result = state
        .dispatch_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            "observe persisted Core rekey",
        )
        .await
        .unwrap();
    let MapResult::TransactionCreated { tx_id } = result else { panic!("observer transaction") };
    let observer = state.runtime().session().get_transaction(&tx_id).unwrap();
    let successor: HolonReference =
        observer.lookup().get_saved_holon_by_key(&"Rekeyed.CoreDefinition".into()).unwrap().into();
    assert_eq!(successor.holon_id().unwrap(), HolonId::Local(successor_id));
    assert_eq!(successor.key().unwrap().unwrap().0, "Rekeyed.CoreDefinition");
    let source = HolonReference::smart_from_id(observer.space_read_handle(), source_id);
    assert_eq!(source.key().unwrap().unwrap().0, "HolonType.TypeDescriptor");
    let meta_type: HolonReference = observer
        .lookup()
        .get_saved_holon_by_key(&"MetaTypeDescriptor.HolonType".into())
        .unwrap()
        .into();
    let diagnosis = holons_core::ExtendsLineageDiagnosis::assess(&meta_type).unwrap();
    let lineage = diagnosis.valid_lineage().expect("saved meta-type retains its valid lineage");
    assert!(lineage.contains(&source).unwrap());
    assert!(!lineage.contains(&successor).unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn loader_core_edit_persists_without_changing_predecessor_contracts() {
    let state = run_core_fixture(loader_core_successor_fixture().unwrap()).await;
    let context = state.context();
    let candidates = context.staged_references().unwrap();
    let update = candidates
        .iter()
        .find(|candidate| candidate.versioned_source_id().unwrap().is_some())
        .expect("promoted saved write source");
    let StagedState::Committed(successor_id) = update.staged_state().unwrap() else {
        panic!("Core edit must persist a successor");
    };
    let source = HolonReference::smart_from_id(
        context.space_read_handle(),
        HolonId::Local(update.versioned_source_id().unwrap().unwrap()),
    );
    assert_eq!(source.key().unwrap().unwrap().0, "HolonType.TypeDescriptor");
    assert!(update.validation_findings().unwrap().is_empty());
    let result = state
        .dispatch_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            "fresh Core successor observer",
        )
        .await
        .unwrap();
    let MapResult::TransactionCreated { tx_id } = result else { panic!("observer transaction") };
    let observer = state.runtime().session().get_transaction(&tx_id).unwrap();
    let source =
        HolonReference::smart_from_id(observer.space_read_handle(), source.holon_id().unwrap());
    let successors = source
        .related_holons_with_hint(
            CoreRelationshipTypeName::Successor,
            RelationshipReadHint::RequireFresh,
        )
        .unwrap();
    let successors = successors.read().unwrap().get_members().to_vec();
    assert_eq!(successors.len(), 1);
    let successor =
        HolonReference::smart_from_id(observer.space_read_handle(), HolonId::Local(successor_id));
    assert!(holons_core::same_definition(&successors[0], &successor).unwrap());
    let predecessors = successor
        .related_holons_with_hint(
            CoreRelationshipTypeName::Predecessor,
            RelationshipReadHint::RequireFresh,
        )
        .unwrap();
    let predecessors = predecessors.read().unwrap().get_members().to_vec();
    assert_eq!(predecessors.len(), 1);
    assert!(holons_core::same_definition(&predecessors[0], &source).unwrap());
    let successor_members = successor
        .related_holons_with_hint(
            CoreRelationshipTypeName::InstanceProperties,
            RelationshipReadHint::RequireFresh,
        )
        .unwrap();
    assert!(successor_members
        .read()
        .unwrap()
        .get_members()
        .iter()
        .any(|member| member.key().unwrap().is_some_and(|key| key.0 == "Title.PropertyType")));
    let book: HolonReference =
        observer.lookup().get_saved_holon_by_key(&"Book.HolonType".into()).unwrap().into();
    let diagnosis = holons_core::ExtendsLineageDiagnosis::assess(&book).unwrap();
    let lineage = diagnosis.valid_lineage().expect("saved Book lineage remains valid");
    assert!(lineage.contains(&source).unwrap());
    assert!(!lineage.contains(&successor).unwrap());
    // This ordinary instance has no abstract-descriptor exemption. Its successful
    // Commit without Title depends on Person retaining the predecessor contract.
    let carrier: HolonReference =
        observer.lookup().get_saved_holon_by_key(&"CoreEdit.Carrier".into()).unwrap().into();
    assert!(carrier.property_value("Title").unwrap().is_none());
    let person: HolonReference =
        observer.lookup().get_saved_holon_by_key(&"Person.HolonType".into()).unwrap().into();
    let holons_core::DescribingTypeResolution::Unique(descriptor) =
        holons_core::resolve_describing_type(&carrier).unwrap()
    else {
        panic!("committed carrier must have one describer");
    };
    assert!(holons_core::same_definition(&descriptor, &person).unwrap());
    let diagnosis = holons_core::ExtendsLineageDiagnosis::assess(&person).unwrap();
    let lineage = diagnosis.valid_lineage().expect("saved Person lineage remains valid");
    assert!(lineage.contains(&source).unwrap());
    assert!(!lineage.contains(&successor).unwrap());
    let saved_members = source
        .related_holons_with_hint(
            CoreRelationshipTypeName::InstanceProperties,
            RelationshipReadHint::RequireFresh,
        )
        .unwrap();
    assert!(!saved_members
        .read()
        .unwrap()
        .get_members()
        .iter()
        .any(|member| member.key().unwrap().is_some_and(|key| key.0 == "Title.PropertyType")));
    assert!(candidates
        .iter()
        .all(|candidate| matches!(candidate.staged_state().unwrap(), StagedState::Committed(_))));
}
