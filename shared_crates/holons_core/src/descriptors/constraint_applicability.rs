use super::definition_identity::lineage_contains;
use super::{
    effective_relationship_targets_with_reader, walk_extends_chain_with_reader,
    CurrentDescriptorReader, DescriptorReader,
};
use crate::reference_layer::HolonReference;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

/// Reads applicability under the kernel's relationship inheritance policy (Local).
pub fn applicable_descriptor_types(
    constraint_type: &HolonReference,
) -> Result<Vec<HolonReference>, HolonError> {
    applicable_descriptor_types_with_reader(constraint_type, &CurrentDescriptorReader)
}

/// Resolves applicability declarations from prospective content.
pub fn applicable_descriptor_types_with_reader<R: DescriptorReader>(
    constraint_type: &HolonReference,
    reader: &R,
) -> Result<Vec<HolonReference>, R::Error> {
    Ok(effective_relationship_targets_with_reader(
        constraint_type,
        CoreRelationshipTypeName::ApplicableToDescriptorTypes,
        reader,
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
    constraint_applies_to_with_reader(constraint_type, descriptor, &CurrentDescriptorReader)
}

/// Tests applicability in the same prospective view as structural and binding assessment.
pub fn constraint_applies_to_with_reader<R: DescriptorReader>(
    constraint_type: &HolonReference,
    descriptor: &HolonReference,
    reader: &R,
) -> Result<bool, R::Error> {
    let lineage =
        walk_extends_chain_with_reader(descriptor, reader).collect::<Result<Vec<_>, _>>()?;
    for target in applicable_descriptor_types_with_reader(constraint_type, reader)? {
        if lineage_contains(&lineage, &target) {
            return Ok(true);
        }
    }
    Ok(false)
}
