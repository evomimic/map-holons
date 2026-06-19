use crate::descriptors::inheritance::flatten_related_members;
use crate::descriptors::{accessor_helpers, OperatorDescriptor, TypeHeader};
use crate::reference_layer::HolonReference;
use base_types::BaseValue;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

fn base_value_kind(value: &BaseValue) -> &'static str {
    match value {
        BaseValue::StringValue(_) => "String",
        BaseValue::BooleanValue(_) => "Boolean",
        BaseValue::IntegerValue(_) => "Integer",
        BaseValue::EnumValue(_) => "Enum",
        BaseValue::BytesValue(_) => "Bytes",
    }
}

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
        found: base_value_kind(found).to_string(),
        descriptor: accessor_helpers::descriptor_label(holon),
    }
}

pub(crate) fn unsupported_operator(
    holon: &HolonReference,
    operator_descriptor: &OperatorDescriptor,
) -> Result<bool, HolonError> {
    Err(unsupported_operator_error(holon, operator_descriptor)?)
}

fn unsupported_operator_error(
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
    Ok(flatten_related_members(holon, CoreRelationshipTypeName::AffordsOperator)?
        .into_iter()
        .map(OperatorDescriptor::from_holon)
        .collect())
}

pub(crate) fn supports_operator(
    holon: &HolonReference,
    operator_descriptor: &OperatorDescriptor,
) -> Result<bool, HolonError> {
    let operator_name = operator_descriptor.operator_name()?;
    for supported_operator in supported_operators(holon)? {
        if supported_operator.operator_name()? == operator_name {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn require_supported_operator(
    holon: &HolonReference,
    operator_descriptor: &OperatorDescriptor,
) -> Result<(), HolonError> {
    // Descriptor affordance gate: subtype wrappers are public execution surfaces,
    // so they enforce the same applicability contract as ValueDescriptor.
    if supports_operator(holon, operator_descriptor)? {
        return Ok(());
    }

    Err(unsupported_operator_error(holon, operator_descriptor)?)
}

pub(crate) fn type_name_is(
    operator_descriptor: &OperatorDescriptor,
    expected: &str,
) -> Result<bool, HolonError> {
    Ok(operator_descriptor.operator_name()?.0 .0 == expected)
}
