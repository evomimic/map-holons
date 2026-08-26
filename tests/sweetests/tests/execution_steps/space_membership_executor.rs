use holons_prelude::prelude::*;
use holons_test::{ResolveBy, SpaceMembershipExpectation, TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::info;
use type_names::CoreRelationshipTypeName;

/// Verifies holon space membership by traversing the persisted relationship graph.
///
/// No index is involved at any point. The space is reached by following `anchor_token`'s
/// `OwnedBy`, so this asserts the membership graph on its own terms — `GetAllHolons` still reads
/// `AllHolonNodes` here, and cannot even be called after a delete, because the index still lists
/// the deleted holon and resolving it fails.
///
/// Both directions are checked, because commit writes them as a pair and a half-written pair is
/// the failure worth catching:
///
/// - the space's `Owns` collection holds exactly `expected_member_keys`, and
/// - every member's `OwnedBy` points back at that same space, with cardinality 1.
///
/// The two sides are read differently, and deliberately. `OwnedBy` is a *declared* name, so
/// `related_holons` resolves it on an undescribed holon. `Owns` is its *inverse*, and the space
/// anchor carries no `DescribedBy` for that resolution to go through — a targeted
/// `related_holons(Owns)` comes back empty even when the link is present. `all_related_holons` is
/// descriptor-unaware and expands the SmartLinks directly, which is what this assertion wants.
pub async fn execute_verify_space_membership(
    state: &mut TestExecutionState,
    anchor_token: TestReference,
    expected: SpaceMembershipExpectation,
) {
    let context =
        state.open_assertion_context("verify_space_membership").await.unwrap_or_else(|error| {
            panic!("verify_space_membership: failed to open assertion transaction: {error:?}")
        });

    // Reach the space by following the anchor's own membership edge.
    let anchor: HolonReference = state
        .resolve_execution_reference(&context, ResolveBy::Expected, &anchor_token)
        .expect("verify_space_membership: failed to resolve the anchor holon");

    let anchor_owners = anchor
        .related_holons(CoreRelationshipTypeName::OwnedBy)
        .expect("verify_space_membership: failed to read the anchor's OwnedBy");
    let space_reference = {
        let owners = anchor_owners.read().expect("verify_space_membership: OwnedBy read lock");
        let members = owners.get_members();
        assert_eq!(
            1,
            members.len(),
            "the anchor holon must have exactly one owner, got {}",
            members.len()
        );
        members[0].clone()
    };

    let space_id = space_reference.holon_id().expect("verify_space_membership: space holon_id");

    // ---- space --Owns--> members -------------------------------------------------------------
    let owns_name = CoreRelationshipTypeName::Owns.as_relationship_name();
    let space_relationships = space_reference
        .all_related_holons()
        .expect("verify_space_membership: failed to read the space's relationships")
        .iter();

    let members: Vec<HolonReference> = space_relationships
        .into_iter()
        .find(|(name, _)| *name == owns_name)
        .map(|(_, collection)| {
            collection
                .read()
                .expect("verify_space_membership: Owns read lock")
                .get_members()
                .to_vec()
        })
        .unwrap_or_default();

    match &expected {
        SpaceMembershipExpectation::ExactKeys(expected_member_keys) => {
            let mut actual_keys: Vec<String> = members
                .iter()
                .map(|member| {
                    member
                        .key()
                        .expect("verify_space_membership: failed to read an Owns member key")
                        .expect("verify_space_membership: an Owns member has no key")
                        .0
                })
                .collect();
            actual_keys.sort();

            let mut expected_keys: Vec<String> =
                expected_member_keys.iter().map(|key| key.0.clone()).collect();
            expected_keys.sort();

            assert_eq!(
                expected_keys, actual_keys,
                "space Owns membership did not match expectations"
            );
        }
        SpaceMembershipExpectation::ExactCount(expected_member_count) => {
            assert_eq!(
                expected_member_count.0,
                members.len() as i64,
                "space Owns membership count did not match expectations"
            );
        }
    }

    // ---- each member --OwnedBy--> the same space ---------------------------------------------
    for member in &members {
        let member_key = member.key().unwrap().unwrap();

        let owned_by_collection =
            member.related_holons(CoreRelationshipTypeName::OwnedBy).unwrap_or_else(|error| {
                panic!("verify_space_membership: OwnedBy read failed for {member_key:?}: {error:?}")
            });
        let owned_by =
            owned_by_collection.read().expect("verify_space_membership: OwnedBy read lock");
        let owners = owned_by.get_members();

        assert_eq!(
            1,
            owners.len(),
            "expected exactly one owner for {member_key:?} (OwnedBy is 1..1), got {}",
            owners.len()
        );
        assert_eq!(
            space_id,
            owners[0].holon_id().expect("verify_space_membership: owner holon_id"),
            "{member_key:?} is owned by a different holon than the current space",
        );
    }

    info!("Success! Space membership matched for {} member(s)", members.len());
}
