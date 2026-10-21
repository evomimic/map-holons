//! Graph-only siblings compose supported additions and reject unsupported overlap before writes.
use super::described_instances::add_described_instance;
use core_types::CommitValidationViolationKind;
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, ExpectedCommitCandidate, ExpectedCommitStatus, ExpectedDisposition,
    ExpectedLineage, ExpectedPersistedGraph, ExpectedPersistedRelationship, ExpectedRejectedHolon,
    ExpectedValidationFinding, ExpectedValidationSubject, PersistedSubject, TestCaseInit,
};
use integrity_core_types::HolonErrorKind;

pub fn commit_disjoint_graph_additions_fixture() -> Result<DancesTestCase, HolonError> {
    graph_siblings("Disjoint", false, false, None)
}
pub fn commit_identical_graph_additions_fixture() -> Result<DancesTestCase, HolonError> {
    graph_siblings("Identical", true, false, None)
}
pub fn commit_graph_overlap_rejection_fixture() -> Result<DancesTestCase, HolonError> {
    graph_siblings("RemovalConflict", false, true, None)
}
pub fn commit_graph_then_version_fixture() -> Result<DancesTestCase, HolonError> {
    graph_siblings("GraphThenVersion", false, false, Some(1))
}
pub fn commit_version_then_graph_fixture() -> Result<DancesTestCase, HolonError> {
    graph_siblings("VersionThenGraph", false, false, Some(0))
}

fn graph_siblings(
    label: &str,
    identical: bool,
    remove: bool,
    version_index: Option<usize>,
) -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        format!("Graph-only siblings {label}"),
        "Independent candidates address one saved source without snapshot substitution",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let mut source = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        &format!("Book.GraphSiblings.{label}"),
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    let mut targets = Vec::new();
    for key in ["Title.PropertyType", "Name.PropertyType"] {
        let key = MapString(key.into());
        let stub = fixture_context.mutation().new_holon(Some(key.clone()))?;
        targets.push(test_case.add_lookup_saved_holon_by_key_step(
            &mut fixture_holons,
            stub,
            key,
            None,
            None,
        )?);
    }
    if remove {
        source = test_case.add_add_related_holons_step(
            &mut fixture_holons,
            source,
            "ReferencesProperty".to_relationship_name(),
            vec![targets[0].clone()],
            None,
            None,
        )?;
    }
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_begin_transaction_step(None, None)?;
    // An explicitly staged unchanged target exercises Commit's NoAction target anchoring.
    let unchanged_target = if version_index.is_some() {
        let target = test_case.add_stage_new_version_step(
            &mut fixture_holons,
            targets[0].clone(),
            None,
            MapInteger(1),
            None,
            None,
        )?;
        targets[0] = target.clone();
        Some(target)
    } else {
        None
    };
    let first = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        source.clone(),
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let second = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        source.clone(),
        None,
        MapInteger(2),
        Some(HolonErrorKind::DuplicateError),
        None,
    )?;
    let mut updates = vec![first, second];
    for (i, update) in updates.iter_mut().enumerate() {
        let target = targets[if identical { 0 } else { i }].clone();
        *update = if remove && i == 0 {
            test_case.add_remove_related_holons_step(
                &mut fixture_holons,
                update.clone(),
                "ReferencesProperty".to_relationship_name(),
                vec![target],
                None,
                None,
            )?
        } else {
            test_case.add_add_related_holons_step(
                &mut fixture_holons,
                update.clone(),
                "ReferencesProperty".to_relationship_name(),
                vec![target],
                None,
                None,
            )?
        };
        if version_index == Some(i) {
            *update = test_case.add_with_properties_step(
                &mut fixture_holons,
                update.clone(),
                [("Title".to_property_name(), "Independent successor".to_base_value())].into(),
                None,
                None,
            )?;
        }
    }
    if remove {
        test_case.add_commit_step(
            &mut fixture_holons,
            ExpectedCommitStatus::Rejected,
            None,
            None,
        )?;
        test_case.add_verify_commit_rejection_step(
            updates
                .iter()
                .map(|token| ExpectedRejectedHolon {
                    token: token.clone(),
                    validation_state: ValidationState::Invalid,
                    findings: vec![ExpectedValidationFinding {
                        kind: CommitValidationViolationKind::RuleViolation {
                            code: "UnsupportedGraphOnlyCombination".into(),
                        },
                        rule_key: None,
                        subject: ExpectedValidationSubject::Holon,
                    }],
                })
                .collect(),
            MapInteger(2),
            None,
        )?;
    } else {
        test_case.add_commit_step_with_dispositions(
            &mut fixture_holons,
            ExpectedCommitStatus::Complete,
            updates
                .iter()
                .enumerate()
                .map(|(i, token)| {
                    ExpectedCommitCandidate::new(
                        token.clone(),
                        if version_index == Some(i) {
                            ExpectedDisposition::NewVersion
                        } else {
                            ExpectedDisposition::GraphOnly
                        },
                    )
                })
                .chain(unchanged_target.iter().map(|target| {
                    ExpectedCommitCandidate::new(target.clone(), ExpectedDisposition::NoAction)
                }))
                .collect(),
            vec![],
            None,
            None,
        )?;
        test_case.add_match_saved_content_step()?;
    }
    let saved_source = PersistedSubject::Token(source.clone());
    let source_targets = if remove {
        vec![targets[0].clone()]
    } else if let Some(version) = version_index {
        vec![targets[1 - version].clone()]
    } else if identical {
        vec![targets[0].clone()]
    } else {
        targets.clone()
    };
    let mut graph = ExpectedPersistedGraph {
        relationships: vec![ExpectedPersistedRelationship {
            source: saved_source.clone(),
            relationship: "ReferencesProperty".to_relationship_name(),
            targets: source_targets.into_iter().map(PersistedSubject::Token).collect(),
        }],
        lineage: vec![ExpectedLineage {
            subject: saved_source,
            predecessors: vec![],
            successors: version_index
                .map(|i| vec![PersistedSubject::Token(updates[i].clone())])
                .unwrap_or_default(),
        }],
        ..Default::default()
    };
    if let Some(version) = version_index {
        graph.relationships.push(ExpectedPersistedRelationship {
            source: PersistedSubject::Token(updates[version].clone()),
            relationship: "ReferencesProperty".to_relationship_name(),
            targets: vec![PersistedSubject::Token(targets[version].clone())],
        });
        graph.lineage.push(ExpectedLineage {
            subject: PersistedSubject::Token(updates[version].clone()),
            predecessors: vec![PersistedSubject::Token(source)],
            successors: vec![],
        });
    }
    test_case.add_verify_persisted_graph_step(
        &fixture_holons,
        graph,
        Some(
            "Inspect the saved source and successor independently from a fresh transaction".into(),
        ),
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
