//! Commit-preparation assessment for independently authored inverse input.

use core_types::{CommitValidationViolationKind, HolonError, ValidationSubjectPath};
use holons_core::descriptors::{RelationshipOccurrenceOrientation, SourceRelationshipContract};
use holons_core::{Descriptor, HolonReference, ReadableHolon, StagedReference};

use crate::{handlers, ValidationCollector};

/// Adds fixed preparation findings for recognized inverse occurrences in a
/// candidate's local staged relationship map.
///
/// The source contract is resolved once per candidate. Missing or multiple
/// source descriptors are left to ordinary holon validation, which owns the
/// `NoDescriptor` finding and its precedence. A target without a descriptor is
/// classified as unresolved by the shared classifier and remains subject to
/// existing undeclared-input behavior.
pub(crate) fn assess_relationship_input(
    candidate: &StagedReference,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let source = HolonReference::from(candidate);
    let contract = match SourceRelationshipContract::resolve(source.clone()) {
        Ok(contract) => contract,
        Err(HolonError::MissingDescribedBy { .. })
        | Err(HolonError::MultipleDescribedBy { .. }) => return Ok(()),
        Err(error) => return Err(error),
    };

    let relationship_map = source.all_related_holons()?;
    for (name, collection) in relationship_map.iter() {
        let members = collection
            .read()
            .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
            .get_members()
            .to_vec();
        for target in members {
            let RelationshipOccurrenceOrientation::RecognizedInverse {
                inverse,
                declared,
                declared_source_type,
            } = contract.classify_occurrence(&name, &target)?
            else {
                continue;
            };

            let subject = ValidationSubjectPath::Relationship {
                source_identity: source.reference_id_string(),
                name: name.to_string(),
                target_identity: target.reference_id_string(),
            };
            handlers::finding(
                collector,
                CommitValidationViolationKind::IndependentlyAuthoredInverseRelationship,
                None,
                &subject,
                Some(inverse.holon().reference_id_string()),
                format!(
                    "Relationship '{}' is the inverse of declared relationship '{}' from source type {}. Author '{}' from that declared source endpoint instead.",
                    name,
                    declared.base_relationship_name()?,
                    declared_source_type.holon().reference_id_string(),
                    declared.base_relationship_name()?,
                ),
            );
        }
    }

    Ok(())
}
