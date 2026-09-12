use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, ExpectedCommitStatus, TestCaseInit};
use integrity_core_types::HolonErrorKind;
use rstest::*;
// use tracing::debug;

use super::described_instances::add_described_instance;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_DESCRIPTOR_KEY,
    STAGE_NEW_VERSION_BOOK_KEY, STAGE_NEW_VERSION_PERSON_1_KEY,
};

// TODO: add/remove relationships

/// Fixture for creating Simple NEWVERSION Testcase
///
/// Schema-backed setup (issues #442/#515): strict commit Pass 2 only persists
/// relationships the source holon's effective schema surface declares. The Book
/// is therefore described by the loaded `Book.HolonType`, whose Extends chain
/// reaches the core `HolonType` relationships used for graph-only and lineage
/// assertions. Commit-time relationship persistence must anchor graph-only
/// changes to the existing Book node and version-producing changes to the new
/// Book version.
///
/// A second graph-only cycle replays the already-persisted `ReferencesProperty` edge
/// (issue #516): commit-time SmartLink duplicate suppression must absorb the
/// equivalent re-write, leaving exactly one forward and one inverse link.
#[fixture]
pub fn stage_new_version_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, fixture_bindings: _ } =
        TestCaseInit::new("Simple StageNewVersion Testcase", "Tests stage_new_version dance");
    let staged_versions_with_same_base_key = MapInteger(1);

    // Operational Core is already available from first-space bootstrap. Load
    // only the test schema that declares Book.HolonType.
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for Book/Person setup".to_string()),
    )?;

    // Only the Book and its eventual author participate in this scenario.
    let book_staged_token = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        STAGE_NEW_VERSION_BOOK_KEY,
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    let person_1_token = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        STAGE_NEW_VERSION_PERSON_1_KEY,
        "Name",
        PERSON_DESCRIPTOR_KEY,
    )?;
    let title_property_stub =
        fixture_context.mutation().new_holon(Some(MapString("Title.PropertyType".to_string())))?;
    let title_property_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        title_property_stub,
        MapString("Title.PropertyType".to_string()),
        None,
        None,
    )?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit --- after setup_book_authors".to_string()),
    )?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // Begin a fresh transaction for a graph-only relationship mutation.
    // `ReferencesProperty` is a non-definitional relationship declared by
    // Book.HolonType, so this must not
    // create a new Book node; it should persist against the existing Book source.
    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction before graph-only Book relationship update".to_string()),
    )?;

    let graph_only_update = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        book_staged_token.clone(),
        None,
        staged_versions_with_same_base_key.clone(),
        None,
        Some("Stage Book as graph-only update context".to_string()),
    )?;
    let graph_only_update = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        graph_only_update,
        RelationshipName(MapString("ReferencesProperty".to_string())),
        vec![title_property_token],
        None,
        Some("Add non-definitional Book --ReferencesProperty--> Title.PropertyType".to_string()),
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit graph-only Book relationship update".to_string()),
    )?;

    // Replay the already-persisted graph-only edge to prove commit-time SmartLink
    // idempotency (issue #516). `stage_new_version` clones all persisted
    // relationships into the staged map, so appending a second ReferencesProperty target
    // marks the relationship touched; the graph-only commit then re-persists the
    // whole collection, including a SmartLink equivalent to the already-persisted
    // Book --ReferencesProperty--> Title.PropertyType edge. Duplicate suppression must
    // leave exactly one link per direction for that edge, asserted by the
    // relationship-anchoring verification step below.
    test_case.add_begin_transaction_step(
        None,
        Some(
            "Begin new transaction before replaying the persisted ReferencesProperty edge"
                .to_string(),
        ),
    )?;

    let replay_update = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        graph_only_update.clone(),
        None,
        staged_versions_with_same_base_key.clone(),
        None,
        Some("Stage Book as replay graph-only update context".to_string()),
    )?;

    // Fresh saved-key lookup so the target binds to this transaction (issue #515).
    let name_property_stub =
        fixture_context.mutation().new_holon(Some(MapString("Name.PropertyType".to_string())))?;
    let name_property_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        name_property_stub,
        MapString("Name.PropertyType".to_string()),
        None,
        None,
    )?;

    test_case.add_add_related_holons_step(
        &mut fixture_holons,
        replay_update,
        RelationshipName(MapString("ReferencesProperty".to_string())),
        vec![name_property_token],
        None,
        Some(
            "Append Book --ReferencesProperty--> Name.PropertyType beside the cloned Title member"
                .to_string(),
        ),
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some(
            "Commit replayed graph-only edge --- expecting SmartLink duplicate suppression"
                .to_string(),
        ),
    )?;

    // Begin a fresh transaction before the definitional relationship mutation.
    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction before version-producing Book update".to_string()),
    )?;

    let staged_clone = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        graph_only_update.clone(),
        None,
        staged_versions_with_same_base_key.clone(),
        None,
        Some("Stage Book for version-producing update".to_string()),
    )?;

    // Add properties
    let mut expected_clone_properties = PropertyMap::new();
    expected_clone_properties.insert(
        "Key".to_property_name(),
        MapString(STAGE_NEW_VERSION_BOOK_KEY.to_string()).to_base_value(),
    );
    expected_clone_properties.insert("Title".to_property_name(), "Changed".to_base_value());

    let staged_clone = test_case.add_with_properties_step(
        &mut fixture_holons,
        staged_clone,
        expected_clone_properties.clone(),
        None,
        Some("With Properties -- first version cloned from book.".to_string()),
    )?;
    // Reuse the setup-phase staged Person1 token directly: the relationship adder
    // resolves it to Person1's committed head (issue #556), so no saved-key lookup
    // workaround is needed for the cross-transaction target (formerly issue #515).
    test_case.add_add_related_holons_step(
        &mut fixture_holons,
        staged_clone,
        RelationshipName(MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string())),
        vec![person_1_token],
        None,
        Some("Add definitional Book --AuthoredBy--> Person relationship".to_string()),
    )?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit --- after staging new first version".to_string()),
    )?;

    test_case.add_verify_relationship_anchoring_step(None)?;

    // Begin fresh transaction so versions 2/3 stage into a clean nursery
    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction after second commit".to_string()),
    )?;

    // VERSION 2 //
    // Stage a second version from the same original holon in order to verify that:
    // a. get_staged_holon_by_base_key returns an error (>1 staged holon with that key)
    // b. get_staged_holons_by_base_key correctly returns BOTH staged holons

    let _version_2_token = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        graph_only_update.clone(),
        None,
        staged_versions_with_same_base_key.clone(),
        None,
        Some(
            "Stage New Version --- second version; first in this transaction, no duplicate"
                .to_string(),
        ),
    )?;

    // Third version in same transaction — now 2 staged holons share the base key
    let staged_in_this_tx = MapInteger(2);

    let _version_3_token = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        book_staged_token,
        None,
        staged_in_this_tx,
        Some(HolonErrorKind::DuplicateError),
        Some("Stage New Version --- third version, expecting DuplicateError from get_staged_holon_by_base_key".to_string()),
    )?;

    // Finalize
    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
