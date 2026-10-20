//! Shared mechanics for an immutable Commit assessment, never a persistence plan.
use crate::{handlers::finding, ValidationCollector};
use core_types::{CommitValidationViolationKind, HolonError, ValidationSubjectPath};
use holons_core::{HolonReference, ReadableHolon};
use type_names::CoreRelationshipTypeName;

/// Findings keep their existing string shape but carry complete temporary IDs for routing.
/// Display abbreviations are suitable for messages, never for candidate associations.
pub(crate) fn identity(subject: &HolonReference) -> String {
    match subject {
        HolonReference::Staged(reference) => format!("TemporaryId={}", reference.temporary_id().0),
        HolonReference::Transient(reference) => {
            format!("TemporaryId={}", reference.temporary_id().0)
        }
        HolonReference::Smart(_) => subject.reference_id_string(),
    }
}

pub(crate) fn path(subject: &HolonReference) -> ValidationSubjectPath {
    ValidationSubjectPath::Holon { holon_identity: identity(subject) }
}

pub(crate) fn blocked(
    collector: &mut ValidationCollector,
    subject: &HolonReference,
    message: String,
) {
    finding(
        collector,
        CommitValidationViolationKind::UnresolvedLocalDependency,
        None,
        &path(subject),
        None,
        message,
    );
}

/// Snapshot bound references before traversing their targets.
pub(crate) fn targets(
    subject: &HolonReference,
    edge: CoreRelationshipTypeName,
) -> Result<Vec<HolonReference>, HolonError> {
    Ok(subject
        .related_holons(edge)?
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
        .get_members()
        .to_vec())
}
