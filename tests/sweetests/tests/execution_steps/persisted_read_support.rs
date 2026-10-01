//! Persisted reads and relationship assertions shared by dance verification steps.

use holons_prelude::prelude::*;
use holons_test::TestExecutionState;
use integrity_core_types::LocalId;
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use pretty_assertions::assert_eq;
use std::sync::Arc;

/// Keeps run-scoped descriptor anchors and returned subjects in the same assertion transaction.
pub(super) async fn loaded_holons_with_context(
    state: &mut TestExecutionState,
    step_name: &str,
) -> (Arc<TransactionContext>, HolonCollection) {
    let context = state.open_assertion_context(step_name).await.unwrap_or_else(|error| {
        panic!("{step_name}: failed to open assertion transaction: {error:?}")
    });

    load_holons(state, context, step_name).await
}

/// Always opens an observer transaction, including while a Commit retry transaction is open.
/// Saved references are rebound to this observer; staged and cached fixture content is not read.
pub(super) async fn loaded_holons_with_fresh_context(
    state: &mut TestExecutionState,
    step_name: &str,
) -> (Arc<TransactionContext>, HolonCollection) {
    let result = state
        .dispatch_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            &format!("{step_name}: begin_fresh_assertion_transaction"),
        )
        .await
        .unwrap_or_else(|error| panic!("{step_name}: failed to open fresh observer: {error:?}"));
    let MapResult::TransactionCreated { tx_id } = result else {
        panic!("{step_name}: expected TransactionCreated, got {result:?}");
    };
    let context =
        state.runtime().session().get_transaction(&tx_id).unwrap_or_else(|error| {
            panic!("{step_name}: failed to resolve fresh observer: {error:?}")
        });
    load_holons(state, context, step_name).await
}

async fn load_holons(
    state: &mut TestExecutionState,
    context: Arc<TransactionContext>,
    step_name: &str,
) -> (Arc<TransactionContext>, HolonCollection) {
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::GetAllHolons,
    });
    let result = state
        .dispatch_command(command, step_name)
        .await
        .unwrap_or_else(|error| panic!("{step_name}: get_all_holons failed: {error:?}"));

    match result {
        MapResult::Collection(collection) => (context, collection),
        other => panic!("{step_name}: expected Collection, got {other:?}"),
    }
}

pub(super) fn find_holon_by_key(holons: &HolonCollection, key: &str) -> HolonReference {
    holons
        .get_by_key(&MapString::from(key))
        .unwrap_or_else(|error| panic!("key lookup for {key} failed: {error:?}"))
        .unwrap_or_else(|| panic!("expected loaded holon with key {key}"))
}

pub(super) fn find_holons_by_key(holons: &HolonCollection, key: &str) -> Vec<HolonReference> {
    holons
        .get_members()
        .iter()
        .filter(|holon| {
            holon
                .key()
                .unwrap_or_else(|error| {
                    panic!("key read failed while searching for {key}: {error:?}")
                })
                .as_ref()
                .map(|actual| actual.0.as_str() == key)
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

pub(super) fn local_id(holon: &HolonReference) -> LocalId {
    match holon.holon_id().unwrap_or_else(|error| panic!("holon_id read failed: {error:?}")) {
        HolonId::Local(local_id) => local_id,
        HolonId::External(external_id) => panic!("expected local holon id, got {external_id:?}"),
    }
}

pub(super) fn string_property(holon: &HolonReference, property_name: &str) -> Option<String> {
    match holon
        .property_value(&PropertyName(MapString::from(property_name)))
        .unwrap_or_else(|error| panic!("property_value({property_name}) failed: {error:?}"))
    {
        Some(BaseValue::StringValue(value)) => Some(value.0),
        Some(other) => panic!("property {property_name} expected string value, got {other:?}"),
        None => None,
    }
}

/// Returns the related holons themselves, for assertions that need to traverse onward
/// from a target rather than just check that its id is present.
pub(super) fn related_holon_members(
    holon: &HolonReference,
    relationship_name: &str,
) -> Vec<HolonReference> {
    let members_handle = holon
        .related_holons(RelationshipName(MapString::from(relationship_name)))
        .unwrap_or_else(|error| panic!("related_holons({relationship_name}) failed: {error:?}"));
    let members = members_handle.read().unwrap_or_else(|error| {
        panic!("related_holons({relationship_name}) lock failed: {error:?}")
    });

    members.get_members().to_vec()
}

pub(super) fn related_holon_ids(holon: &HolonReference, relationship_name: &str) -> Vec<LocalId> {
    let members_handle = holon
        .related_holons(RelationshipName(MapString::from(relationship_name)))
        .unwrap_or_else(|error| panic!("related_holons({relationship_name}) failed: {error:?}"));
    let members = members_handle.read().unwrap_or_else(|error| {
        panic!("related_holons({relationship_name}) lock failed: {error:?}")
    });

    members.get_members().iter().map(local_id).collect()
}

pub(super) fn assert_related_ids_contain(
    holon: &HolonReference,
    relationship_name: &str,
    expected: &LocalId,
) {
    let ids = related_holon_ids(holon, relationship_name);
    assert!(
        ids.iter().any(|actual| actual == expected),
        "expected relationship {relationship_name} ids {ids:?} to contain {expected:?}"
    );
}

pub(super) fn assert_related_ids_do_not_contain(
    holon: &HolonReference,
    relationship_name: &str,
    unexpected: &LocalId,
) {
    let ids = related_holon_ids(holon, relationship_name);
    assert!(
        ids.iter().all(|actual| actual != unexpected),
        "expected relationship {relationship_name} ids {ids:?} not to contain {unexpected:?}"
    );
}

/// Asserts the target id appears exactly once in the persisted relationship —
/// duplicate SmartLinks would surface here as repeated members, because the
/// guest fetch path adds one collection member per persisted link.
pub(super) fn assert_related_ids_contain_exactly_once(
    holon: &HolonReference,
    relationship_name: &str,
    expected: &LocalId,
) {
    let ids = related_holon_ids(holon, relationship_name);
    let occurrences = ids.iter().filter(|actual| *actual == expected).count();
    assert_eq!(
        occurrences, 1,
        "expected relationship {relationship_name} ids {ids:?} to contain {expected:?} exactly once"
    );
}

/// Bypasses relationship cache policy for a direct persisted traversal through the reference layer.
pub(super) fn fresh_related_holon_members(
    holon: &HolonReference,
    relationship_name: &str,
) -> Vec<HolonReference> {
    let members = holon
        .related_holons_with_hint(
            relationship_name,
            holons_core::RelationshipReadHint::RequireFresh,
        )
        .unwrap_or_else(|error| {
            panic!("fresh related_holons({relationship_name}) failed: {error:?}")
        });
    let members = members.read().unwrap_or_else(|error| {
        panic!("fresh related_holons({relationship_name}) lock failed: {error:?}")
    });
    members.get_members().to_vec()
}

/// Reads every persisted occurrence, preserving duplicates for exact identity assertions.
pub(super) fn fresh_related_holon_ids(
    holon: &HolonReference,
    relationship_name: &str,
) -> Vec<LocalId> {
    fresh_related_holon_members(holon, relationship_name).iter().map(local_id).collect()
}
