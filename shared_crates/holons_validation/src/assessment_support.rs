//! Shared mechanics for an immutable prospective assessment, never a persistence plan.
use crate::{handlers::finding, ValidationCollector};
use core_types::{CommitValidationViolationKind, HolonError, ValidationSubjectPath};
use holons_core::{AssessmentReadError, HolonReference, ReadableHolon};
use type_names::CoreRelationshipTypeName;

pub(crate) fn path(subject: &HolonReference) -> ValidationSubjectPath {
    ValidationSubjectPath::Holon { holon_identity: subject.reference_id_string() }
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

/// Only content contention is recoverable here. An unreadable graph invalidates the entire pass.
pub(crate) fn recover<T>(
    result: Result<T, AssessmentReadError>,
    subject: &HolonReference,
    collector: &mut ValidationCollector,
) -> Result<Option<T>, HolonError> {
    recover_at(result, &path(subject), collector)
}

/// Shared anchor failures have no primary holon; the transaction carrier owns them.
pub(crate) fn recover_transaction<T>(
    result: Result<T, AssessmentReadError>,
    collector: &mut ValidationCollector,
) -> Result<Option<T>, HolonError> {
    recover_at(result, &ValidationSubjectPath::Transaction, collector)
}

fn recover_at<T>(
    result: Result<T, AssessmentReadError>,
    subject: &ValidationSubjectPath,
    collector: &mut ValidationCollector,
) -> Result<Option<T>, HolonError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error @ AssessmentReadError::Contested { .. }) => {
            finding(
                collector,
                CommitValidationViolationKind::UnresolvedLocalDependency,
                None,
                subject,
                None,
                error.to_string(),
            );
            Ok(None)
        }
        Err(AssessmentReadError::Operational(error)) => Err(error),
        Err(error @ AssessmentReadError::SchemaIncompatible { .. }) => {
            Err(HolonError::CommitFailure(error.to_string()))
        }
    }
}

/// Snapshot local references before selection, preserving contested identities for diagnostics.
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
