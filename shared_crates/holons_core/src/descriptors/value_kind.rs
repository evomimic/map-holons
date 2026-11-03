//! Foundational value classification shared by preparation and bootstrap validation.
use crate::{
    descriptors::{TypeHeader, ValueDescriptorKind},
    HolonReference,
};
use base_types::BaseValueKind;
use core_types::HolonError;

/// Kernel-only classification for validation before derived runtime schema is available.
/// Ordinary semantic reads use ValueDescriptor::value_kind and its prepared products.
/// This primitive performs no derived construction and does not validate conformance.
/// Ordinary consumers use ValueDescriptor::value_kind so registered/frozen products
/// are reused. This kernel path is reserved for bootstrap before result schemas exist.
pub fn classify_value_kind_for_validation(
    target: &HolonReference,
) -> Result<ValueDescriptorKind, HolonError> {
    classify_value_kind_in_lineage(target, super::inheritance::walk_extends_chain(target))
}

/// Classifies using an already resolved lineage; never follows another Extends edge.
pub(crate) fn classify_value_kind_in_lineage(
    target: &HolonReference,
    lineage: impl IntoIterator<Item = Result<HolonReference, HolonError>>,
) -> Result<ValueDescriptorKind, HolonError> {
    for ancestor in lineage {
        let ancestor = ancestor?;
        if TypeHeader::new(&ancestor).defines_instance_type_kind()? {
            return classify_value_kind_anchor(&ancestor);
        }
    }
    Err(missing_value_kind(target))
}

/// Prepares both kind products while distinguishing unreadable content from
/// readable-but-malformed declarations. Only the latter may be memoized as errors.
pub(crate) fn prepare_kind_products(
    target: &HolonReference,
    lineage: &[HolonReference],
) -> Result<(Result<Option<usize>, HolonError>, Result<ValueDescriptorKind, HolonError>), HolonError>
{
    use crate::ReadableHolon;
    use base_types::BaseValue;
    use type_names::CorePropertyTypeName as P;
    let mut selected = Ok(None);
    for (index, ancestor) in lineage.iter().enumerate() {
        selected = match ancestor.property_value(P::DefinesInstanceTypeKind)? {
            Some(BaseValue::BooleanValue(value)) if value.0 => Ok(Some(index)),
            Some(BaseValue::BooleanValue(_)) => continue,
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{other:?}"), "Boolean".into()))
            }
            None => Err(HolonError::EmptyField("DefinesInstanceTypeKind".into())),
        };
        break;
    }
    let native = match &selected {
        Ok(Some(index)) => match lineage[*index].property_value(P::TypeName)? {
            Some(BaseValue::StringValue(name)) => Ok(value_kind_from_name(&name.0)),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{other:?}"), "String".into()))
            }
            None => Err(HolonError::EmptyField("TypeName".into())),
        },
        Ok(None) => Err(missing_value_kind(target)),
        Err(error) => Err(error.clone()),
    };
    Ok((selected, native))
}

fn classify_value_kind_anchor(anchor: &HolonReference) -> Result<ValueDescriptorKind, HolonError> {
    Ok(value_kind_from_name(&TypeHeader::new(anchor).type_name()?.0))
}

fn value_kind_from_name(name: &str) -> ValueDescriptorKind {
    use BaseValueKind::*;
    match name {
        "StringValueType" => ValueDescriptorKind::BaseValue(String),
        "IntegerValueType" => ValueDescriptorKind::BaseValue(Integer),
        "BooleanValueType" => ValueDescriptorKind::BaseValue(Boolean),
        "BytesValueType" => ValueDescriptorKind::BaseValue(Bytes),
        "EnumValueType" | "MapEnumValueType" => ValueDescriptorKind::BaseValue(Enum),
        "BaseValueValueType" | "ValueType" => ValueDescriptorKind::AnyBaseValue,
        "ValueArrayValueType" | "MapValueArrayType" => ValueDescriptorKind::ValueArray,
        _ => ValueDescriptorKind::Unsupported(name.to_string()),
    }
}

pub(crate) fn missing_value_kind(target: &HolonReference) -> HolonError {
    HolonError::WrongDescriptorKind {
        expected: "IntegerValueType, StringValueType, BooleanValueType, BytesValueType, EnumValueType, BaseValueValueType, or ValueArrayValueType".into(),
        found: "No instance TypeKind anchor".into(),
        descriptor: super::accessor_helpers::descriptor_label(target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        descriptors::test_support::{build_context, new_descriptor_holon},
        WritableHolon,
    };

    #[test]
    fn bootstrap_classifier_needs_no_runtime_result_schema_or_inherited_collections(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut anchor =
            new_descriptor_holon(&context, "StringValueType", "StringValueType", "Value")?;
        anchor.with_property_value("DefinesInstanceTypeKind", true)?;
        // An invalid tail must not be followed after the nearest anchor. The
        // full derived constructor separately diagnoses the complete lineage.
        anchor.add_related_holons("Extends", vec![anchor.clone().into()])?;
        assert_eq!(
            classify_value_kind_for_validation(&anchor.into())?,
            ValueDescriptorKind::BaseValue(BaseValueKind::String)
        );
        Ok(())
    }
    #[test]
    fn operational_field_shaped_read_errors_do_not_become_prepared_semantic_errors() {
        use crate::descriptors::test_support::build_context_with_read_errors;
        use core_types::{HolonId, LocalId};
        use std::collections::HashMap;
        let id = HolonId::Local(LocalId(vec![51; 39]));
        let error = HolonError::EmptyField("transport could not read content".into());
        let context = build_context_with_read_errors(
            vec![],
            HashMap::new(),
            HashMap::from([(id.clone(), error.clone())]),
        );
        let source = HolonReference::smart_from_id(context.space_read_handle(), id);
        assert_eq!(prepare_kind_products(&source, &[source.clone()]), Err(error));
    }
}
