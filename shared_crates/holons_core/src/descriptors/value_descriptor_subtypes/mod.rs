mod enum_value_descriptor;
mod integer_value_descriptor;
mod string_value_descriptor;
mod value_array_descriptor;

pub use enum_value_descriptor::EnumValueDescriptor;
pub use integer_value_descriptor::IntegerValueDescriptor;
pub use string_value_descriptor::StringValueDescriptor;
pub use value_array_descriptor::ValueArrayDescriptor;

use crate::descriptors::inheritance::flatten_related_members;
use crate::descriptors::{accessor_helpers, OperatorDescriptor, TypeHeader};
use crate::reference_layer::HolonReference;
use base_types::BaseValue;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

pub(super) fn base_value_kind(value: &BaseValue) -> &'static str {
    match value {
        BaseValue::StringValue(_) => "String",
        BaseValue::BooleanValue(_) => "Boolean",
        BaseValue::IntegerValue(_) => "Integer",
        BaseValue::EnumValue(_) => "Enum",
    }
}

pub(super) fn value_type_name(holon: &HolonReference) -> Result<String, HolonError> {
    Ok(TypeHeader::new(holon).type_name()?.to_string())
}

pub(super) fn value_kind_mismatch(
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

pub(super) fn unsupported_operator(
    holon: &HolonReference,
    op: &OperatorDescriptor,
) -> Result<bool, HolonError> {
    Err(HolonError::UnsupportedOperator {
        operator: op.type_name()?.to_string(),
        value_type: value_type_name(holon)?,
        descriptor: accessor_helpers::descriptor_label(holon),
    })
}

pub(super) fn supported_operators(
    holon: &HolonReference,
) -> Result<Vec<OperatorDescriptor>, HolonError> {
    Ok(flatten_related_members(holon, CoreRelationshipTypeName::AffordsOperator)?
        .into_iter()
        .map(OperatorDescriptor::from_holon)
        .collect())
}

pub(super) fn supports_operator(
    holon: &HolonReference,
    op: &OperatorDescriptor,
) -> Result<bool, HolonError> {
    let operator_name = op.type_name()?;
    for supported in supported_operators(holon)? {
        if supported.type_name()? == operator_name {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn type_name_is(op: &OperatorDescriptor, expected: &str) -> Result<bool, HolonError> {
    Ok(op.type_name()?.0 == expected)
}
