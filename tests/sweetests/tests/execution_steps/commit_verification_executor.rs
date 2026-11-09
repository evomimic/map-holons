//! Fresh persisted checks bounded to Commit's fixture-authored workset.

use super::persisted_read_support::{fresh_assertion_context, fresh_related_holon_ids, local_id};
use holons_core::core_shared_objects::holon::StagedState;
use holons_core::descriptors::TargetBinding;
use holons_core::{Descriptor, EffectiveHolonDescriptor};
use holons_prelude::prelude::*;
use holons_test::{
    assert_edge_occurrences, CommitVerificationBaseline, EdgeExpectation, ExpectedCommitStatus,
    ExpectedDisposition, ExpectedMembershipEdit, ResolvedCommitCandidate, TestExecutionState,
    TestReference,
};
use integrity_core_types::LocalId;
use std::{collections::HashMap, sync::Arc};

fn saved(context: &Arc<TransactionContext>, id: &LocalId) -> HolonReference {
    HolonReference::smart_from_id(context.space_read_handle(), HolonId::Local(id.clone()))
}

/// Resolves a frozen fixture token by correspondence, including a just-committed staged member.
/// Neither key lookup nor final-head redirection can substitute a different version.
fn identity(state: &TestExecutionState, token: &TestReference) -> LocalId {
    let reference = state
        .holons()
        .by_snapshot_id
        .get(&token.expected_id())
        .expect("Commit verification member must have an execution correspondence")
        .execution_handle
        .get_holon_reference()
        .unwrap();
    match reference {
        HolonReference::Smart(_) => local_id(&reference),
        HolonReference::Staged(staged) => match staged.staged_state().unwrap() {
            StagedState::Committed(id) => id,
            StagedState::ForUpdate
            | StagedState::ForUpdateGraphOnly
            | StagedState::ForUpdateNewVersion => staged
                .versioned_source_id()
                .unwrap()
                .expect("unchanged staged target must have a saved source"),
            other => panic!("Commit verification member has no saved realization: {other:?}"),
        },
        _ => panic!("Commit verification requires a persisted member: {token}"),
    }
}

/// Reuses fixture-established expectations. Only unknown named memberships need
/// named pre-write observations; repeated subjects share one observation within the attempt.
pub async fn capture(
    state: &TestExecutionState,
    declarations: &[ResolvedCommitCandidate],
) -> Vec<CommitVerificationBaseline> {
    if declarations.is_empty() {
        return vec![];
    }
    let needs_observer = declarations.iter().any(|candidate| {
        let Some(source) = &candidate.saved_source else {
            return false;
        };
        let id = identity(state, source);
        ["Predecessor".to_string(), "Successor".to_string()]
            .into_iter()
            .chain(candidate.relationships.iter().map(|relationship| relationship.name.to_string()))
            .any(|name| !state.expected_persisted_memberships().contains_key(&(id.clone(), name)))
    });
    let context = if needs_observer {
        Some(fresh_assertion_context(state, "Commit: bounded baseline").await)
    } else {
        None
    };
    let mut observed = HashMap::new();
    let mut baseline = |source: &LocalId, name: &str| {
        let key = (source.clone(), name.to_string());
        if let Some(expected) = state.expected_persisted_memberships().get(&key) {
            return expected.clone();
        }
        observed
            .entry(key)
            .or_insert_with(|| {
                fresh_related_holon_ids(&saved(context.as_ref().unwrap(), source), name)
            })
            .clone()
    };
    let mut baselines = Vec::new();
    for candidate in declarations {
        let source = candidate.saved_source.as_ref().map(|token| identity(state, token));
        let lineage = source
            .as_ref()
            .map(|source| (baseline(source, "Predecessor"), baseline(source, "Successor")));
        let relationships = candidate
            .relationships
            .iter()
            .map(|expected| {
                let baseline = source
                    .as_ref()
                    .map(|source| baseline(source, &expected.name.to_string()))
                    .unwrap_or_default();
                (expected.clone(), baseline)
            })
            .collect();
        baselines.push(CommitVerificationBaseline {
            candidate: candidate.clone(),
            lineage,
            relationships,
        });
    }
    if let Some(context) = context {
        state.runtime().session().archive_transaction(&context.tx_id()).unwrap();
    }
    baselines
}

/// Projects a saved identity according to the exact relationship direction's binding.
fn bound_id(reference: &HolonReference, binding: TargetBinding) -> LocalId {
    match binding {
        TargetBinding::Version => local_id(reference),
        TargetBinding::Lineage => {
            let HolonReference::Smart(reference) = reference else { unreachable!() };
            let HolonId::Local(id) = reference.lineage_root_id().unwrap() else {
                panic!("Commit verification expects local lineage identity")
            };
            id
        }
    }
}

/// Complete attempts verify all retained intentions once, including earlier incomplete writes.
/// Partial attempts retain their pre-write baselines rather than accepting partial writes as intent.
pub async fn verify(
    state: &mut TestExecutionState,
    baselines: Vec<CommitVerificationBaseline>,
    status: ExpectedCommitStatus,
) {
    state.pending_commit_verification().extend(baselines);
    if status != ExpectedCommitStatus::Complete {
        return;
    }
    let plans = std::mem::take(state.pending_commit_verification());
    if plans.is_empty() {
        return;
    }
    let context = fresh_assertion_context(state, "Commit: bounded persisted verification").await;
    let mut exact: HashMap<(LocalId, String), Vec<LocalId>> = HashMap::new();
    // Keep inverse checks separate: unrelated sources can contribute to these collections.
    let mut inverses = Vec::new();
    let mut owned_inverse_keys = std::collections::HashSet::new();
    for plan in &plans {
        let candidate = &plan.candidate;
        // Incomplete NoAction retains its staged token. Its saved source still has
        // to preserve the original pre-attempt graph through any later retry.
        let result = candidate
            .result_token
            .as_ref()
            .or(candidate.saved_source.as_ref())
            .expect("saved verification participant");
        let result_id = identity(state, result);
        let result_ref = saved(&context, &result_id);
        let source_id = candidate.saved_source.as_ref().map(|source| identity(state, source));
        // Preserve known incoming memberships of the touched saved source as well.
        // This uses the fixture ledger, never a graph traversal to discover contributors.
        owned_inverse_keys.extend(
            state
                .fixture_inverse_contributors()
                .keys()
                .filter(|(id, _)| *id == result_id || source_id.as_ref() == Some(id))
                .cloned(),
        );
        if let Some(source) = &candidate.saved_source {
            let source_id = identity(state, source);
            let (predecessors, successors) = plan.lineage.as_ref().unwrap();
            exact
                .entry((source_id.clone(), "Predecessor".into()))
                .or_insert_with(|| predecessors.clone());
            let expected_successors = exact
                .entry((source_id.clone(), "Successor".into()))
                .or_insert_with(|| successors.clone());
            if candidate.disposition == ExpectedDisposition::NewVersion {
                if !expected_successors.contains(&result_id) {
                    expected_successors.push(result_id.clone());
                }
                exact.insert((result_id.clone(), "Predecessor".into()), vec![source_id]);
                exact.insert((result_id.clone(), "Successor".into()), vec![]);
            }
        } else {
            exact.insert((result_id.clone(), "Predecessor".into()), vec![]);
            exact.insert((result_id.clone(), "Successor".into()), vec![]);
        }
        if matches!(
            candidate.disposition,
            ExpectedDisposition::NewRoot | ExpectedDisposition::NewVersion
        ) {
            state.fixture_saved_identities().insert(result_id.clone());
            // A successor inherits none of its source's fixture-known incoming memberships.
            // Discovering every inverse definition would expand preparation beyond the workset.
            if let Some(source_id) = &source_id {
                let names: Vec<_> = state
                    .fixture_inverse_contributors()
                    .keys()
                    .filter(|(id, _)| id == source_id)
                    .map(|(_, name)| name.clone())
                    .collect();
                for name in names {
                    let key = (result_id.clone(), name);
                    state.fixture_inverse_contributors().entry(key.clone()).or_default();
                    owned_inverse_keys.insert(key);
                }
            }
        }
        if plan.relationships.is_empty() {
            continue;
        }
        let descriptor = result_ref.holon_descriptor().unwrap();
        let effective = EffectiveHolonDescriptor::acquire(descriptor.holon().clone()).unwrap();
        // Prepared declaration roles avoid re-deriving direction for every instance.
        let declared: HashMap<_, _> = effective
            .instance_relationships()
            .unwrap()
            .into_iter()
            .map(|relationship| (relationship.base_relationship_name().unwrap(), relationship))
            .collect();
        for (expected, baseline) in &plan.relationships {
            let policy = declared
                .get(&expected.name)
                .expect("fixture Commit memberships must use declared relationships");
            let binding = policy.target_binding().unwrap();
            let ids = |tokens: &[TestReference]| {
                tokens
                    .iter()
                    .map(|token| bound_id(&saved(&context, &identity(state, token)), binding))
                    .collect::<Vec<_>>()
            };
            let before = ids(&expected.before);
            let after = ids(&expected.after);
            let name = expected.name.to_string();
            if candidate.disposition == ExpectedDisposition::NewVersion {
                let source_id = identity(state, candidate.saved_source.as_ref().unwrap());
                exact.entry((source_id, name.clone())).or_insert_with(|| baseline.clone());
            }
            let members =
                exact.entry((result_id.clone(), name.clone())).or_insert_with(|| baseline.clone());
            holons_test::apply_fixture_membership_change(members, &before, &after);
            let mut touched: std::collections::HashSet<_> =
                before.iter().chain(after.iter()).cloned().collect();
            // An explicit removal/replacement on a partial lookup is intent even when
            // the removed member was absent from its fixture stub.
            for edit in &expected.edits {
                let targets = match edit {
                    ExpectedMembershipEdit::Add(targets)
                    | ExpectedMembershipEdit::Remove(targets)
                    | ExpectedMembershipEdit::Replace(targets) => ids(targets),
                };
                touched.extend(targets.iter().cloned());
                match edit {
                    ExpectedMembershipEdit::Add(_) => {
                        for target in targets {
                            if !members.contains(&target) {
                                members.push(target);
                            }
                        }
                    }
                    ExpectedMembershipEdit::Remove(_) => {
                        members.retain(|target| !targets.contains(target))
                    }
                    ExpectedMembershipEdit::Replace(_) => {
                        touched.extend(members.iter().cloned());
                        *members = targets;
                    }
                }
            }
            if let Some(inverse) = policy.has_inverse().unwrap() {
                let inverse_binding = inverse.target_binding().unwrap();
                let inverse_source = bound_id(&result_ref, inverse_binding);
                let inverse_name = inverse.base_relationship_name().unwrap().to_string();
                for target in touched {
                    if candidate.disposition == ExpectedDisposition::NewVersion {
                        let source_id = identity(state, candidate.saved_source.as_ref().unwrap());
                        let inverse_source_id =
                            bound_id(&saved(&context, &source_id), inverse_binding);
                        inverses.push((
                            source_id,
                            name.clone(),
                            target.clone(),
                            inverse_name.clone(),
                            inverse_source_id,
                            inverse_binding,
                        ));
                    }
                    inverses.push((
                        result_id.clone(),
                        name.clone(),
                        target,
                        inverse_name.clone(),
                        inverse_source.clone(),
                        inverse_binding,
                    ));
                }
            }
        }
    }
    let mut inverse_expectations: HashMap<(LocalId, String, LocalId), (bool, TargetBinding)> =
        HashMap::new();
    for (source, name, target, inverse, inverse_source, binding) in inverses {
        let present = exact[&(source.clone(), name)].contains(&target);
        let key = (target.clone(), inverse.clone());
        if state.fixture_saved_identities().contains(&target) {
            state.fixture_inverse_contributors().entry(key.clone()).or_default();
        }
        if let Some(contributors) = state.fixture_inverse_contributors().get_mut(&key) {
            if present {
                contributors.insert(source, inverse_source.clone());
            } else {
                contributors.remove(&source);
            }
            owned_inverse_keys.insert(key);
        }
        let entry = inverse_expectations
            .entry((target, inverse, inverse_source))
            .or_insert((false, binding));
        entry.0 |= present;
    }
    for key in owned_inverse_keys {
        let contributors = &state.fixture_inverse_contributors()[&key];
        let members: std::collections::HashSet<_> = contributors.values().cloned().collect();
        exact.insert(key, members.into_iter().collect());
    }
    for ((source, name), expected) in &exact {
        // Preserve duplicate occurrences: converting actual membership to a set would hide bugs.
        let mut actual = fresh_related_holon_ids(&saved(&context, source), name);
        let mut expected = expected.clone();
        actual.sort_by(|a, b| a.0.cmp(&b.0));
        expected.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(actual, expected, "Commit persisted membership {source:?} --{name}-->");
    }
    tracing::info!(
        "Commit persisted verification: {} candidates, {} named collections, {} inverse occurrences",
        plans.len(), exact.len(), inverse_expectations.len(),
    );
    let mut inverse_reads = HashMap::new();
    for ((target, inverse, inverse_source), (present, binding)) in inverse_expectations {
        // A lineage-bound inverse can retain contributions from versions outside this
        // workset. Absence needs an explicit fixture expectation; never expand the read
        // scope to discover all other versions merely to justify an automatic check.
        if !present && binding == TargetBinding::Lineage {
            continue;
        }
        let key = (target.clone(), inverse.clone());
        if exact.contains_key(&key) {
            continue;
        }
        let actual = inverse_reads
            .entry(key)
            .or_insert_with(|| fresh_related_holon_ids(&saved(&context, &target), &inverse));
        assert_edge_occurrences(
            &format!("Commit inverse {target:?} --{inverse}--> {inverse_source:?}"),
            actual,
            &inverse_source,
            if present { EdgeExpectation::ExactlyOnce } else { EdgeExpectation::Absent },
        );
    }
    state.retain_persisted_membership_expectations(exact);
    state.runtime().session().archive_transaction(&context.tx_id()).unwrap();
}
