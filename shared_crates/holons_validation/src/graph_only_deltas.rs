//! Capability checks for overlapping graph-only writes, without saved collection reads.
use crate::{assessment_support::path, handlers::finding, ValidationCollector};
use core_types::{
    CommitValidationViolationKind, HolonError, HolonId, LocalId, RelationshipName, TemporaryId,
};
use holons_core::{
    core_shared_objects::{
        holon::{RelationshipCommitScope, StagedState},
        pending_relationship_deltas::{PendingDelta, PendingRelationshipTarget},
        transactions::TransactionContext,
        Holon,
    },
    descriptors::TargetBinding,
    DefinitionIdentity, HolonReference, LineageIdentity, ReadableHolon, StagedReference,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

struct Participant {
    reference: StagedReference,
    live: bool,
    deltas: BTreeMap<RelationshipName, PendingDelta>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum PersistedTarget {
    Version(DefinitionIdentity),
    Lineage(LineageIdentity),
}

/// Rejects unsupported overlap before persistence, including outstanding graph-only replay.
/// Pending changes belong to saved source buckets; successor snapshots never participate.
/// Reads only mutation summaries and added targets, never saved relationship collections.
pub(crate) fn classify_graph_only_delta_overlaps(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    for (_, (source, participants)) in collect_participants(context, candidates)? {
        let mut buckets: BTreeMap<RelationshipName, Vec<usize>> = BTreeMap::new();
        for (i, participant) in participants.iter().enumerate() {
            for (name, delta) in &participant.deltas {
                if !delta.is_empty() {
                    buckets.entry(name.clone()).or_default().push(i);
                }
            }
        }
        for (name, indices) in buckets {
            if indices.len() > 1 && overlap_is_unsupported(context, &participants, &name, &indices)?
            {
                let source_identity = HolonReference::smart_from_id(
                    context.space_read_handle(),
                    HolonId::Local(source.clone()),
                )
                .full_reference_id_string();
                report_unsupported_overlap(
                    &source_identity,
                    &participants,
                    &name,
                    &indices,
                    collector,
                );
            }
        }
    }
    Ok(())
}

type ParticipantGroups = BTreeMap<Vec<u8>, (LocalId, Vec<Participant>)>;

/// Includes committed graph-only entries whose recorded deltas remain eligible for replay.
fn collect_participants(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
) -> Result<ParticipantGroups, HolonError> {
    let live_ids: HashSet<TemporaryId> = candidates.iter().map(|c| c.temporary_id()).collect();
    let mut by_source: BTreeMap<Vec<u8>, (LocalId, Vec<Participant>)> = BTreeMap::new();
    for reference in context.staged_references()? {
        let rc = reference.get_holon_to_commit(context)?;
        let holon = rc.read().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        let Holon::Staged(staged) = &*holon else { continue };
        let live = live_ids.contains(&reference.temporary_id());
        let source = match staged.staged_state() {
            StagedState::ForUpdateGraphOnly if live => staged.get_versioned_source_id()?,
            StagedState::Committed(id)
                if staged.relationship_commit_scope() == RelationshipCommitScope::TouchedOnly
                    && staged.versioned_source_id_ref() == Some(id) =>
            {
                id.clone()
            }
            _ => continue,
        };
        let deltas = staged.pending_relationship_deltas().clone();
        by_source
            .entry(source.0.clone())
            .or_insert_with(|| (source, Vec::new()))
            .1
            .push(Participant { reference, live, deltas });
    }
    for (_, participants) in by_source.values_mut() {
        participants.sort_by_key(|p| HolonReference::from(&p.reference).full_reference_id_string());
    }
    Ok(by_source)
}

/// Determines whether the recorded instructions define compatible put-only writes.
fn overlap_is_unsupported(
    context: &Arc<TransactionContext>,
    participants: &[Participant],
    name: &RelationshipName,
    indices: &[usize],
) -> Result<bool, HolonError> {
    if indices.iter().any(|i| {
        let delta = &participants[*i].deltas[name];
        delta.repeated_occurrence || !delta.removed.is_empty()
    }) {
        return Ok(true);
    }
    // Resolve the same forward descriptor that Pass 2 uses. No target population is read.
    let subject: HolonReference = (&participants[indices[0]].reference).into();
    let relationship = subject.holon_descriptor()?.get_relationship_by_name(name.clone());
    let relationship = match relationship {
        // Ungoverned assembly can bypass the mutation-time declaration check.
        // Keep this semantic defect available to ordinary subject assessment.
        Err(HolonError::DescriptorDeclarationNotFound { .. }) => return Ok(true),
        other => other?,
    };
    let binding = relationship.try_into_declared_relationship_descriptor()?.target_binding()?;
    let mut writes: HashMap<PersistedTarget, (DefinitionIdentity, Option<base_types::MapString>)> =
        HashMap::new();
    let mut unsupported = false;
    for i in indices {
        let participant = &participants[*i];
        let subject: HolonReference = (&participant.reference).into();
        let members = subject
            .related_holons(name.clone())?
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .get_members()
            .clone();
        for id in &participant.deltas[name].added {
            let target = members
                .iter()
                .find(|target| PendingRelationshipTarget::for_reference(target) == *id)
                .ok_or_else(|| {
                    let message = format!(
                        "Pending addition {id:?} absent from candidate {} relationship {name}",
                        subject.full_reference_id_string(),
                    );
                    HolonError::InvalidState(message)
                })?;
            let definition = DefinitionIdentity::for_reference(target, context)?;
            let persisted = match binding {
                TargetBinding::Version => PersistedTarget::Version(definition.clone()),
                TargetBinding::Lineage => {
                    PersistedTarget::Lineage(LineageIdentity::for_reference(target, context)?)
                }
            };
            let write = (definition, target.key()?);
            if let Some(existing) = writes.get(&persisted) {
                if existing != &write {
                    unsupported = true;
                }
            } else {
                writes.insert(persisted, write);
            }
        }
    }
    Ok(unsupported)
}

/// Attaches one finding per live participant; retained committed entries own no new outcome.
fn report_unsupported_overlap(
    source_identity: &str,
    participants: &[Participant],
    name: &RelationshipName,
    indices: &[usize],
    collector: &mut ValidationCollector,
) {
    for i in indices {
        let participant = &participants[*i];
        if !participant.live {
            continue;
        }
        let subject: HolonReference = (&participant.reference).into();
        let others = indices
            .iter()
            .filter(|j| *j != i)
            .map(|j| {
                let other = &participants[*j];
                let id = HolonReference::from(&other.reference).full_reference_id_string();
                if other.live {
                    id
                } else {
                    format!("{id} (outstanding replay from an incomplete Commit)")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let message = format!(
            "Candidate {} has an unsupported graph-only combination on saved source {source_identity}, \
             relationship {name}, with {others}. Undeclared relationships, overlapping removals, \
             repeated occurrences, or incompatible writes to one persisted target cannot be composed.",
            subject.full_reference_id_string(),
        );
        finding(
            collector,
            CommitValidationViolationKind::RuleViolation {
                code: "UnsupportedGraphOnlyCombination".into(),
            },
            None,
            &path(&subject),
            None,
            message,
        );
    }
}
