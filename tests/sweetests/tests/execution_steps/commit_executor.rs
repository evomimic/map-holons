use core_types::ValidationSubjectPath;
use holons_core::core_shared_objects::holon::{StagedState, ValidationState};
use holons_test::{
    ExecutionHandle, ExecutionReference, ExpectedCommitStatus, ExpectedRejectedHolon,
    ExpectedValidationSubject, ResolveBy, TestExecutionState, TestReference,
};
use integrity_core_types::HolonErrorKind;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use std::collections::BTreeMap;
use tracing::{debug, info, trace};

use holons_prelude::prelude::*;

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
                    assert_eq!(
                        *validation_state,
                        ValidationState::ValidationRequired,
                        "these abandoned fixtures were never assessed"
                    );
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
                let rejected = commit_response_ref
                    .related_holons(CoreRelationshipTypeName::RejectedHolons)
                    .expect("RejectedHolons relationship");
                assert!(
                    rejected.read().unwrap().get_count().0 > 0,
                    "rejection must identify finding-bearing candidates"
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
    assert_eq!(finding_count, expected_violation_count.0);
}
