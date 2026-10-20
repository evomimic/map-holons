use crate::descriptors::inheritance::effective_relationship_targets;
use crate::descriptors::{
    accessor_helpers, lineage_contains, Descriptor, OperatorDescriptor, ResolvedOperator,
    TypeHeader,
};
use crate::reference_layer::HolonReference;
use base_types::BaseValue;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

pub(crate) fn value_type_name(holon: &HolonReference) -> Result<String, HolonError> {
    Ok(TypeHeader::new(holon).type_name()?.to_string())
}

pub(crate) fn value_kind_mismatch(
    holon: &HolonReference,
    expected: &str,
    found: &BaseValue,
) -> HolonError {
    HolonError::ValueKindMismatch {
        expected: expected.to_string(),
        found: found.kind().to_string(),
        descriptor: accessor_helpers::descriptor_label(holon),
    }
}

pub(crate) fn unsupported_operator(
    holon: &HolonReference,
    operator_descriptor: &OperatorDescriptor,
) -> Result<bool, HolonError> {
    Err(unsupported_operator_error(holon, operator_descriptor)?)
}

pub(crate) fn unsupported_operator_error(
    holon: &HolonReference,
    operator_descriptor: &OperatorDescriptor,
) -> Result<HolonError, HolonError> {
    Ok(HolonError::UnsupportedOperator {
        operator: operator_descriptor.operator_name()?.0.to_string(),
        value_type: value_type_name(holon)?,
        descriptor: accessor_helpers::descriptor_label(holon),
    })
}

pub(crate) fn supported_operators(
    holon: &HolonReference,
) -> Result<Vec<OperatorDescriptor>, HolonError> {
    Ok(effective_relationship_targets(holon, CoreRelationshipTypeName::AffordsOperator)?
        .into_iter()
        .map(|member| OperatorDescriptor::from_holon(member.member))
        .collect())
}

/// Returns whether `holon` affords this exact operator descriptor.
///
/// Affordance is by descriptor identity: an unrelated descriptor sharing an
/// afforded operator's name is not afforded, and an extension operator must be
/// afforded itself rather than through its ancestor.
pub(crate) fn supports_operator(
    holon: &HolonReference,
    operator_descriptor: &OperatorDescriptor,
) -> Result<bool, HolonError> {
    let supported = supported_operators(holon)?;
    lineage_contains(supported.iter().map(|operator| operator.holon()), operator_descriptor.holon())
}

/// Returns the single authored operand of a binary operator.
pub(crate) fn single_operand<'a>(
    op: &ResolvedOperator,
    operands: &'a [BaseValue],
) -> Result<&'a BaseValue, HolonError> {
    match operands {
        [operand] => Ok(operand),
        _ => Err(HolonError::OperandCountMismatch {
            operator: op.operator().operator_name()?.to_string(),
            expected: 1,
            found: operands.len(),
        }),
    }
}
