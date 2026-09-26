use core_types::ValidationSubjectPath;
use holons_core::core_shared_objects::holon::{StagedState, ValidationState};
use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedCommitCarrierFinding, ExpectedCommitStatus,
    ExpectedRejectedHolon, ExpectedValidationSubject, ResolveBy, TestExecutionState, TestReference,
};
use integrity_core_types::HolonErrorKind;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::{debug, info, trace};

use holons_prelude::prelude::*;

type RelationshipSnapshot = HashMap<String, HashMap<HolonId, usize>>;
type PersistenceSnapshot = (HashSet<HolonId>, HashMap<HolonId, RelationshipSnapshot>);

/// The persisted nodes and saved relationship endpoints reachable from a rejected
/// workset. A semantic rejection must leave both unchanged, including inverses.
fn persistence_snapshot(
    context: &std::sync::Arc<TransactionContext>,
    candidates: &[&StagedReference],
) -> Result<PersistenceSnapshot, HolonError> {
    let nodes = context
        .lookup()
        .get_all_holons()?
        .get_members()
        .iter()
        .map(|reference| reference.holon_id())
        .collect::<Result<HashSet<_>, _>>()?;
    let mut saved_endpoints = HashMap::new();
    for candidate in candidates {
        if let Some(source) = candidate.versioned_source_id()? {
            let id = HolonId::Local(source);
            saved_endpoints
                .insert(id.clone(), HolonReference::smart_from_id(context.space_read_handle(), id));
        }
        for (_, members) in HolonReference::from(*candidate).all_related_holons()?.iter() {
            let members = members
                .read()
                .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?;
            for member in members.get_members() {
                if let HolonReference::Smart(saved) = member {
                    saved_endpoints.insert(saved.holon_id(), member.clone());
                }
            }
        }
    }
    let mut relationships = HashMap::new();
    for (id, saved) in saved_endpoints {
        let mut edges = RelationshipSnapshot::new();
        for (name, members) in saved.all_related_holons()?.iter() {
            let mut targets = HashMap::new();
            let members = members
                .read()
                .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?;
            for member in members.get_members() {
                *targets.entry(member.holon_id()?).or_insert(0) += 1;
            }
            edges.insert(name.to_string(), targets);
        }
        relationships.insert(id, edges);
    }
    Ok((nodes, relationships))
}

/// Dispatches a `Commit` command through the Runtime and validates the result.
///
/// Asserts the `CommitRequestStatus` on the commit response against
/// `expected_status`. An `Incomplete` commit is still an `Ok` response with
/// potentially saved holons, so saved-holon registration proceeds for both
/// statuses. `Rejected` instead retains staged handles and performs no saved registration.
///
/// On success, reads committed holons from the `SavedHolons` relationship on the
/// commit response holon and registers them in the test execution state.
pub async fn execute_commit(
    state: &mut TestExecutionState,
    expected_tokens: Vec<TestReference>,
    expected_status: ExpectedCommitStatus,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state.context();
    state.set_last_commit_response(None);

    // Retain staged handles across the wire round trip, including entries that
    // Commit excludes. This checks workset accounting and outcome replacement
    // without following fixture heads to their newly saved references.
    let staged = context.staged_references().expect("complete Nursery");
    let candidates: Vec<_> = staged
        .iter()
        .filter(|reference| reference.is_live_validation_candidate().unwrap())
        .collect();
    let before_rejection = (expected_status == ExpectedCommitStatus::Rejected)
        .then(|| persistence_snapshot(&context, &candidates).expect("pre-Commit persisted state"));
    let excluded: Vec<_> = staged
        .iter()
        .filter(|reference| !reference.is_live_validation_candidate().unwrap())
        .map(|reference| {
            (
                reference,
                reference.validation_state().unwrap(),
                reference.validation_findings().unwrap(),
                reference.is_in_state(&context, StagedState::Abandoned).unwrap(),
            )
        })
        .collect();

    // 1. BUILD — transaction commit command
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::Commit,
    });

    // 2. DISPATCH
    let result = state.dispatch_command(command, "commit").await;
    debug!("Commit result: {:?}", &result);

    // 3. VALIDATE
    match result {
        Ok(MapResult::Reference(HolonReference::Transient(commit_response_ref))) => {
            assert!(expected_error.is_none(), "commit succeeded but expected {:?}", expected_error,);

            let actual_status = match commit_response_ref
                .property_value(&CorePropertyTypeName::CommitRequestStatus)
            {
                Ok(Some(PropertyValue::StringValue(MapString(status)))) => status,
                Ok(other) => panic!(
                    "commit: expected string CommitRequestStatus on commit response, got {:?}",
                    other
                ),
                Err(e) => panic!("commit: failed to read CommitRequestStatus: {:?}", e),
            };
            // A status mismatch is the hardest failure in this suite to diagnose: the assertion
            // compares two strings, while the findings that explain the status live on the staged
            // handles and on the response carrier. Report both before asserting. These use
            // `println!` rather than tracing so that no `RUST_LOG` filter can suppress the
            // explanation of a failure.
            if actual_status != expected_status.to_string() {
                println!("---- commit status mismatch: expected {expected_status}, got {actual_status} ----");
                for candidate in &candidates {
                    println!(
                        "candidate {} state={:?} findings={:#?}",
                        candidate.reference_id_string(),
                        candidate.validation_state().unwrap(),
                        candidate.validation_findings().unwrap(),
                    );
                }
                match commit_response_ref
                    .related_holons(CoreRelationshipTypeName::HasValidationFinding)
                {
                    Ok(carriers) => println!(
                        "unattached carrier findings: {:#?}",
                        carriers.read().unwrap().get_members()
                    ),
                    Err(error) => println!("carrier findings unavailable: {error:?}"),
                }
                println!("---- end commit status mismatch ----");
            }
            assert_eq!(
                actual_status,
                expected_status.to_string(),
                "Expected CommitRequestStatus={}, got {}",
                expected_status,
                actual_status
            );
            info!("Success! Commit completed via Runtime dispatch with status {}", actual_status);
            state.set_last_commit_response(Some(commit_response_ref.clone()));
            assert_eq!(
                commit_response_ref.property_value(CorePropertyTypeName::CommitsAttempted).unwrap(),
                Some(BaseValue::IntegerValue(MapInteger(candidates.len() as i64))),
                "only live validation candidates count as attempts"
            );
            for (reference, validation_state, findings, abandoned) in &excluded {
                assert_eq!(reference.validation_state().unwrap(), *validation_state);
                assert_eq!(reference.validation_findings().unwrap(), *findings);
                if *abandoned {
                    assert!(reference.is_in_state(&context, StagedState::Abandoned).unwrap());
                }
            }
            if expected_status != ExpectedCommitStatus::Rejected {
                assert_eq!(
                    commit_response_ref
                        .property_value(CorePropertyTypeName::ValidationViolationCount)
                        .unwrap(),
                    Some(BaseValue::IntegerValue(MapInteger(0)))
                );
                assert_eq!(
                    commit_response_ref
                        .related_holons(CoreRelationshipTypeName::RejectedHolons)
                        .unwrap()
                        .read()
                        .unwrap()
                        .get_count()
                        .0,
                    0
                );
                assert_eq!(
                    commit_response_ref
                        .related_holons(CoreRelationshipTypeName::HasValidationFinding)
                        .unwrap()
                        .read()
                        .unwrap()
                        .get_count()
                        .0,
                    0,
                    "accepted Commit has no unattached findings"
                );
                for candidate in &candidates {
                    assert_eq!(candidate.validation_state().unwrap(), ValidationState::Validated);
                    assert!(
                        candidate.validation_findings().unwrap().is_empty(),
                        "accepted retry clears stale findings"
                    );
                }
            }

            // 4. GET — committed holons from the SavedHolons relationship
            let committed_references = commit_response_ref
                .related_holons(CoreRelationshipTypeName::SavedHolons)
                .expect("Failed to read SavedHolons relationship");

            let committed_refs_guard = committed_references.read().unwrap();
            let commit_count: MapInteger = committed_refs_guard.get_count();
            debug!("Discovered {:?} committed holons", commit_count.0);

            if expected_status == ExpectedCommitStatus::Rejected {
                assert_eq!(commit_count.0, 0, "rejection must not save holons");
                assert!(context.is_open(), "rejection must leave the transaction open");
                assert_eq!(
                    Some(
                        persistence_snapshot(&context, &candidates)
                            .expect("post-Commit persisted state")
                    ),
                    before_rejection,
                    "semantic rejection must not write nodes or SmartLinks"
                );
                let rejected = commit_response_ref
                    .related_holons(CoreRelationshipTypeName::RejectedHolons)
                    .expect("RejectedHolons relationship");
                let rejected_count = rejected.read().unwrap().get_count().0;
                let carrier_count = commit_response_ref
                    .related_holons(CoreRelationshipTypeName::HasValidationFinding)
                    .expect("HasValidationFinding relationship")
                    .read()
                    .unwrap()
                    .get_count()
                    .0;
                assert!(
                    rejected_count + carrier_count > 0,
                    "rejection must expose at least one finding"
                );
                return;
            }
            assert_eq!(
                commit_count.0 as usize,
                expected_tokens.len(),
                "SavedHolons matches the fixture workset"
            );

            // 5. RECORD — register committed holons so tokens become resolvable
            let holon_collection =
                committed_references.read().expect("Failed to read committed holons");

            // Temporary key-based matching: source token (expected) → resulting reference (actual)
            // TODO: solve or migrate issue 352
            let mut index: usize = 0;
            let mut keyed_index = BTreeMap::new();
            for token in &expected_tokens {
                let key = token.expected_reference().clone().key().unwrap().expect(
                    "For these testing purposes, source token (TestReference) must have a key",
                );
                keyed_index.insert(key, index);
                index += 1;
            }
            for holon_reference in holon_collection.get_members() {
                let source_index = keyed_index
                    .get(
                        &holon_reference.key().unwrap().expect(
                            "For these testing purposes, resulting reference (HolonReference) must have a key",
                        ),
                    )
                    .expect("Expected source token to be indexed by key");
                let token = &expected_tokens[*source_index];
                let execution_handle = ExecutionHandle::from(holon_reference.clone());
                let execution_reference =
                    ExecutionReference::from_token_execution(token, execution_handle);
                state.record(token, execution_reference).unwrap();
            }

            trace!("Commit complete: {} holons committed", committed_refs_guard.get_count().0);
        }
        Err(e) => {
            let actual = HolonErrorKind::from(&e);
            assert_eq!(Some(actual), expected_error, "commit: unexpected error {:?}", e,);
        }
        Ok(other) => panic!("commit: expected Transient reference, got {:?}", other),
    }
}

/// Checks the response relationship against client-side staged handles and their restored findings.
pub fn execute_verify_commit_rejection(
    state: &TestExecutionState,
    rejected_holons: Vec<ExpectedRejectedHolon>,
    expected_violation_count: MapInteger,
) {
    let context = state.context();
    assert!(context.is_open(), "rejected transaction must remain open");
    let response = state.last_commit_response().expect("a preceding Commit response");
    assert_eq!(
        response.property_value(CorePropertyTypeName::CommitRequestStatus).unwrap(),
        Some(BaseValue::StringValue(MapString::from("Rejected")))
    );
    assert_eq!(
        response.property_value(CorePropertyTypeName::ValidationViolationCount).unwrap(),
        Some(BaseValue::IntegerValue(expected_violation_count.clone()))
    );
    let rejected = response.related_holons(CoreRelationshipTypeName::RejectedHolons).unwrap();
    let members = rejected.read().unwrap().get_members().to_vec();
    let mut finding_count = assert_rejected_holons(state, &members, rejected_holons);
    let carriers = response.related_holons(CoreRelationshipTypeName::HasValidationFinding).unwrap();
    let carrier_members = carriers.read().unwrap().get_members().to_vec();
    for carrier in &carrier_members {
        assert!(matches!(carrier, HolonReference::Transient(_)));
        for field in [
            CorePropertyTypeName::ViolationKind,
            CorePropertyTypeName::Severity,
            CorePropertyTypeName::SubjectKind,
            CorePropertyTypeName::Message,
        ] {
            assert!(
                carrier.property_value(field).unwrap().is_some(),
                "finding carrier is incomplete"
            );
        }
    }
    finding_count += carrier_members.len() as i64;
    assert_eq!(finding_count, expected_violation_count.0);
}

/// Checks both the rejected identities and each staged candidate's finding details.
fn assert_rejected_holons(
    state: &TestExecutionState,
    members: &[HolonReference],
    rejected_holons: Vec<ExpectedRejectedHolon>,
) -> i64 {
    let context = state.context();
    let mut actual_ids: Vec<_> =
        members.iter().map(|reference| reference.reference_id_string()).collect();
    let mut expected_ids = Vec::new();
    let mut finding_count = 0;
    for expected in rejected_holons {
        let reference = state
            .resolve_execution_reference(&context, ResolveBy::Expected, &expected.token)
            .expect("staged token remains resolvable");
        let identity = reference.reference_id_string();
        expected_ids.push(identity.clone());
        let HolonReference::Staged(staged) = reference else {
            panic!("rejected token must remain staged")
        };
        assert!(staged.is_live_validation_candidate().unwrap());
        assert_eq!(staged.validation_state().unwrap(), expected.validation_state);
        let findings = staged.validation_findings().unwrap();
        assert!(!findings.is_empty(), "rejected member must carry findings");
        assert_eq!(findings.len(), expected.findings.len());
        finding_count += findings.len() as i64;
        for (actual, expected) in findings.iter().zip(expected.findings) {
            let subject = match expected.subject {
                ExpectedValidationSubject::Holon => {
                    ValidationSubjectPath::Holon { holon_identity: identity.clone() }
                }
                ExpectedValidationSubject::Property(name) => {
                    ValidationSubjectPath::Property { holon_identity: identity.clone(), name }
                }
                ExpectedValidationSubject::Value(property) => {
                    ValidationSubjectPath::Value { holon_identity: identity.clone(), property }
                }
                ExpectedValidationSubject::Relationship { name, target } => {
                    let target = state
                        .resolve_execution_reference(&context, ResolveBy::Expected, &target)
                        .expect("relationship finding target remains resolvable");
                    ValidationSubjectPath::Relationship {
                        source_identity: identity.clone(),
                        name,
                        target_identity: target.reference_id_string(),
                    }
                }
                ExpectedValidationSubject::Transaction => ValidationSubjectPath::Transaction,
            };
            assert_eq!(actual.kind, expected.kind);
            assert_eq!(actual.rule_key, expected.rule_key);
            assert_eq!(actual.subject, subject);
        }
    }
    actual_ids.sort();
    expected_ids.sort();
    assert_eq!(
        actual_ids, expected_ids,
        "RejectedHolons must identify exactly the expected staged candidates"
    );
    finding_count
}

/// Reads the transient carrier exactly as a client would after public Commit.
pub fn execute_verify_commit_carrier_finding(
    state: &TestExecutionState,
    expected: ExpectedCommitCarrierFinding,
) {
    let context = state.context();
    let schema = context
        .lookup()
        .get_saved_holon_by_key(&MapString(expected.schema_key.clone()))
        .expect("expected saved Schema");
    assert!(
        context.staged_references().unwrap().iter().all(|candidate| {
            candidate.key().unwrap() != Some(MapString(expected.schema_key.clone()))
        }),
        "aggregate finding must name an unstaged Schema"
    );
    let response = state.last_commit_response().expect("a preceding rejected Commit response");
    assert_eq!(
        response.property_value(CorePropertyTypeName::CommitRequestStatus).unwrap(),
        Some(BaseValue::StringValue(MapString("Rejected".into())))
    );
    let rejected = response
        .related_holons(CoreRelationshipTypeName::RejectedHolons)
        .expect("RejectedHolons relationship");
    let rejected_members = rejected.read().unwrap().get_members().to_vec();
    let staged_count =
        assert_rejected_holons(state, &rejected_members, expected.expected_rejected_holons);
    let carriers = response
        .related_holons(CoreRelationshipTypeName::HasValidationFinding)
        .expect("HasValidationFinding relationship");
    let matches: Vec<_> = carriers
        .read()
        .unwrap()
        .get_members()
        .iter()
        .filter(|carrier| {
            carrier.property_value(CorePropertyTypeName::RuleCode).unwrap()
                == Some(BaseValue::StringValue(MapString(expected.rule_code.clone())))
                && carrier.property_value(CorePropertyTypeName::HolonIdentity).unwrap()
                    == Some(BaseValue::StringValue(MapString(schema.reference_id_string())))
        })
        .cloned()
        .collect();
    assert_eq!(matches.len(), 1, "one carrier must identify the unstaged Schema and rule");
    let carrier = &matches[0];
    let declaring_descriptor = context
        .lookup()
        .get_saved_holon_by_key(&MapString("Schema.HolonType".into()))
        .expect("saved Schema descriptor");
    let declaring_identity = declaring_descriptor.reference_id_string();
    for (field, value) in [
        (CorePropertyTypeName::ViolationKind, "RuleViolation"),
        (CorePropertyTypeName::RuleIdentity, expected.rule_key.as_str()),
        (CorePropertyTypeName::SubjectKind, "Holon"),
        (CorePropertyTypeName::Severity, "Error"),
        (CorePropertyTypeName::DescriptorIdentity, declaring_identity.as_str()),
    ] {
        assert_eq!(
            carrier.property_value(field).unwrap(),
            Some(BaseValue::StringValue(MapString(value.into())))
        );
    }
    let message = carrier.property_value(CorePropertyTypeName::Message).unwrap();
    assert!(
        matches!(message, Some(BaseValue::StringValue(MapString(ref value))) if !value.is_empty())
    );
    assert_eq!(
        response.property_value(CorePropertyTypeName::ValidationViolationCount).unwrap(),
        Some(BaseValue::IntegerValue(MapInteger(
            staged_count + carriers.read().unwrap().get_count().0
        ))),
        "the transaction count includes staged and carrier findings"
    );
}
