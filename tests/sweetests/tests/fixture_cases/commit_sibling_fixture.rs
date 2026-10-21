//! Commit preserves distinct sibling versions and graph edits anchored to their saved source.

use super::described_instances::add_described_instance;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, ExpectedCommitCandidate, ExpectedCommitStatus, ExpectedDisposition,
    ExpectedLineage, ExpectedPersistedGraph, ExpectedPersistedRelationship, PersistedSubject,
    TestCaseInit,
};
use integrity_core_types::HolonErrorKind;

/// Distinct edits of one saved version publish distinct successors in one Commit.
/// Sharing a source, lineage and key must not collapse either candidate or saved result.
pub fn commit_version_siblings_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit version-producing siblings",
        "Two candidates publish independent versions with the same exact predecessor",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let original = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.VersionSiblings",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_begin_transaction_step(None, None)?;

    let first = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(1),
        None,
        Some("Stage first successor from A".into()),
    )?;
    // A singular base-key lookup is ambiguous, while both candidates remain staged.
    // Stage both before editing so the staging executor can compare their cloned content.
    let second = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(2),
        Some(HolonErrorKind::DuplicateError),
        Some("Stage second successor from the same A".into()),
    )?;
    let first = test_case.add_with_properties_step(
        &mut fixture_holons,
        first,
        [("Title".to_property_name(), "Sibling B title".to_base_value())].into(),
        None,
        None,
    )?;
    let second = test_case.add_with_properties_step(
        &mut fixture_holons,
        second,
        [("Title".to_property_name(), "Sibling C title".to_base_value())].into(),
        None,
        None,
    )?;
    // Both candidates publish their own content. Their shared key belongs to one
    // lineage, so maintaining its saved key associations must permit both writes.
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![
            ExpectedCommitCandidate::new(first.clone(), ExpectedDisposition::NewVersion),
            ExpectedCommitCandidate::new(second.clone(), ExpectedDisposition::NewVersion),
        ],
        vec![],
        None,
        None,
    )?;
    test_case.add_match_saved_content_step()?;
    let a = PersistedSubject::Token(original);
    let b = PersistedSubject::Token(first);
    let c = PersistedSubject::Token(second);
    // Fresh reads require two distinct successors, each pointing to exactly A.
    // Collapsing either result onto A or its sibling fails these exact collections.
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        ExpectedPersistedGraph {
            enumerated: vec![a.clone(), PersistedSubject::Key("Book.VersionSiblings".into())],
            lineage: vec![
                ExpectedLineage {
                    subject: a.clone(),
                    predecessors: vec![],
                    successors: vec![b.clone(), c.clone()],
                },
                ExpectedLineage { subject: b, predecessors: vec![a.clone()], successors: vec![] },
                ExpectedLineage { subject: c, predecessors: vec![a], successors: vec![] },
            ],
            ..Default::default()
        },
        Some("Verify both siblings persist as distinct successors of A".into()),
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// An unchanged candidate and a graph-only candidate share a saved definition,
/// while Commit handles their outcomes separately and writes only the graph edit.
pub fn commit_no_op_and_graph_only_siblings_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit no-op and graph-only siblings",
        "Both candidate results name the saved source and its graph edit creates no version",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let original = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.GraphOnlySiblings",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let property_key = MapString("Title.PropertyType".into());
    let property_stub = fixture_context.mutation().new_holon(Some(property_key.clone()))?;
    let property = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        property_stub,
        property_key,
        None,
        None,
    )?;
    let first = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let unchanged = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(2),
        Some(HolonErrorKind::DuplicateError),
        None,
    )?;
    let graph_only = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        first,
        "ReferencesProperty".to_relationship_name(),
        vec![property.clone()],
        None,
        None,
    )?;
    // NoAction binds its result directly to A without consuming the GraphOnly
    // saved result. The graph write must also remain anchored to A.
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![
            ExpectedCommitCandidate::new(unchanged.clone(), ExpectedDisposition::NoAction),
            ExpectedCommitCandidate::new(graph_only.clone(), ExpectedDisposition::GraphOnly),
        ],
        vec![],
        None,
        None,
    )?;
    test_case.add_match_saved_content_step()?;
    let source_aliases = vec![
        PersistedSubject::Token(original),
        PersistedSubject::Token(unchanged),
        PersistedSubject::Token(graph_only),
    ];
    // Resolve all three tokens through a fresh observer: each must see A's graph,
    // and neither update may persist its staged predecessor edge onto A itself.
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        ExpectedPersistedGraph {
            enumerated: source_aliases.clone(),
            relationships: source_aliases
                .iter()
                .map(|source| ExpectedPersistedRelationship {
                    source: source.clone(),
                    relationship: "ReferencesProperty".to_relationship_name(),
                    targets: vec![PersistedSubject::Token(property.clone())],
                })
                .collect(),
            lineage: source_aliases
                .into_iter()
                .map(|subject| ExpectedLineage {
                    subject,
                    predecessors: vec![],
                    successors: vec![],
                })
                .collect(),
            ..Default::default()
        },
        Some("Verify all saved aliases see the graph edit without a new version".into()),
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// Publishing branches in separate transactions preserves the exact source of each
/// successor, just as publishing siblings together does.
pub fn commit_branch_across_transactions_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit branches across transactions",
        "A to B and A to C both commit when staged in separate transactions",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let original = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.BranchSource",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;

    test_case.add_begin_transaction_step(None, None)?;
    let branch_b = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(1),
        None,
        Some("Stage branch B from A".into()),
    )?;
    let branch_b_title: PropertyMap =
        [("Title".to_property_name(), "Branch B title".to_base_value())].into();
    let branch_b = test_case.add_with_properties_step(
        &mut fixture_holons,
        branch_b,
        branch_b_title,
        None,
        None,
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(branch_b.clone(), ExpectedDisposition::NewVersion)],
        vec![],
        None,
        None,
    )?;

    test_case.add_match_saved_content_step()?;

    test_case.add_begin_transaction_step(None, None)?;
    let branch_c = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        original.clone(),
        None,
        MapInteger(1),
        None,
        Some("Stage branch C from A in a later transaction".into()),
    )?;
    let branch_c_title: PropertyMap =
        [("Title".to_property_name(), "Branch C title".to_base_value())].into();
    let branch_c = test_case.add_with_properties_step(
        &mut fixture_holons,
        branch_c,
        branch_c_title,
        None,
        None,
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(branch_c.clone(), ExpectedDisposition::NewVersion)],
        vec![],
        None,
        None,
    )?;
    test_case.add_match_saved_content_step()?;
    // Tokens distinguish the branches sharing one key; get-all still enumerates only A.
    let a = PersistedSubject::Token(original);
    let b = PersistedSubject::Token(branch_b);
    let c = PersistedSubject::Token(branch_c);
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        ExpectedPersistedGraph {
            enumerated: vec![a.clone(), PersistedSubject::Key("Book.BranchSource".into())],
            lineage: vec![
                ExpectedLineage { subject: a.clone(), predecessors: vec![], successors: vec![b.clone(), c.clone()] },
                ExpectedLineage { subject: b, predecessors: vec![a.clone()], successors: vec![] },
                ExpectedLineage { subject: c, predecessors: vec![a], successors: vec![] },
            ],
            ..Default::default()
        },
        Some("Verify both branches have exactly A as predecessor and A has exactly B and C as successors".into()),
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
