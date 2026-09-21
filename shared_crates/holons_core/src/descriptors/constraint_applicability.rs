use super::definition_identity::lineage_contains;
use super::{ancestors, effective_relationship_targets};
use crate::reference_layer::HolonReference;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

/// Reads applicability under the kernel's relationship inheritance policy (Local).
pub fn applicable_descriptor_types(
    constraint_type: &HolonReference,
) -> Result<Vec<HolonReference>, HolonError> {
    Ok(effective_relationship_targets(
        constraint_type,
        CoreRelationshipTypeName::ApplicableToDescriptorTypes,
    )?
    .into_iter()
    .map(|contribution| contribution.member)
    .collect())
}

/// Tests identity/lineage applicability without invoking an instance evaluator.
pub fn constraint_applies_to(
    constraint_type: &HolonReference,
    descriptor: &HolonReference,
) -> Result<bool, HolonError> {
    let lineage = ancestors(descriptor)?;
    Ok(applicable_descriptor_types(constraint_type)?
        .iter()
        .any(|target| lineage_contains(&lineage, target)))
}
