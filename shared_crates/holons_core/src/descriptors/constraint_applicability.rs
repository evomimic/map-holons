use super::definition_identity::lineage_contains;
use super::{effective_relationship_targets, walk_extends_chain, ValidExtendsLineage};
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
    let lineage = walk_extends_chain(descriptor).collect::<Result<Vec<_>, _>>()?;
    applies_to_members(constraint_type, &lineage)
}

/// Reuses a diagnosed lineage when many constraint attachments share one subject.
/// The supplied lineage and applicability targets must belong to the same input snapshot.
pub fn constraint_applies_to_lineage(
    constraint_type: &HolonReference,
    lineage: ValidExtendsLineage<'_>,
) -> Result<bool, HolonError> {
    applies_to_members(constraint_type, lineage.members())
}

fn applies_to_members(
    constraint_type: &HolonReference,
    lineage: &[HolonReference],
) -> Result<bool, HolonError> {
    for target in applicable_descriptor_types(constraint_type)? {
        if lineage_contains(lineage, &target)? {
            return Ok(true);
        }
    }
    Ok(false)
}
