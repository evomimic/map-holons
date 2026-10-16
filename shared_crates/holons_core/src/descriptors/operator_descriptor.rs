use crate::descriptors::inheritance::effective_relationship_targets;
use crate::descriptors::{
    accessor_helpers, classify_descriptor_lineage, CaseMatching, Descriptor, OperandSpecification,
    OperatorCategory, OperatorKind, ResolvedOperator, TypeHeader, ValueDescriptor,
};
use crate::reference_layer::HolonReference;
use base_types::{BaseValue, MapString};
use core_types::HolonError;
use type_names::{
    operator_type_descriptor_key, CoreOperatorTypeName, CorePropertyTypeName,
    CoreRelationshipTypeName, OperatorName,
};

/// Runtime wrapper for operator descriptors.
///
/// Operators remain schema-declared holons. This wrapper exposes their
/// descriptor-local metadata without introducing a global operator registry.
#[derive(Debug, Clone)]
pub struct OperatorDescriptor {
    holon: HolonReference,
}

impl OperatorDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns the operator descriptor's canonical operator name.
    pub fn operator_name(&self) -> Result<OperatorName, HolonError> {
        Ok(OperatorName(self.header().type_name()?))
    }

    /// Returns the optional human-facing display name.
    pub fn display_name(&self) -> Result<Option<MapString>, HolonError> {
        self.header().display_name()
    }

    /// Returns the optional human-facing description.
    pub fn description(&self) -> Result<Option<MapString>, HolonError> {
        self.header().description()
    }

    /// Returns the declared operand count for this operator.
    pub fn arity(&self) -> Result<u8, HolonError> {
        let arity = accessor_helpers::require_integer(&self.holon, CorePropertyTypeName::Arity)?;
        arity.try_into().map_err(|_| HolonError::IntegerOutOfRange {
            value: arity,
            min: Some(u8::MIN.into()),
            max: Some(u8::MAX.into()),
            min_inclusive: true,
            max_inclusive: true,
            descriptor: "OperatorDescriptor::arity".to_string(),
        })
    }

    /// Returns the parsed operator category enum value.
    pub fn operator_category(&self) -> Result<OperatorCategory, HolonError> {
        let value = accessor_helpers::require_enum_string(
            &self.holon,
            CorePropertyTypeName::OperatorCategory,
        )?;
        OperatorCategory::parse(&value)
    }

    /// Returns the effective case-matching policy, if this operator declares one.
    ///
    /// Resolved self-first through the `Extends` lineage, so an extension of a
    /// matching operator inherits its ancestor's policy unless it declares its own.
    pub fn case_matching(&self) -> Result<Option<CaseMatching>, HolonError> {
        match accessor_helpers::effective_property_value(
            &self.holon,
            CorePropertyTypeName::CaseMatching,
        )? {
            None => Ok(None),
            Some(BaseValue::StringValue(value)) => CaseMatching::parse(&value).map(Some),
            Some(BaseValue::EnumValue(value)) => CaseMatching::parse(&value.0).map(Some),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{other:?}"), "Enum".to_string()))
            }
        }
    }

    /// Returns this operator's authored operand specifications in position order.
    pub fn operand_specifications(&self) -> Result<Vec<OperandSpecification>, HolonError> {
        Ok(effective_relationship_targets(
            &self.holon,
            CoreRelationshipTypeName::OperandSpecifications,
        )?
        .into_iter()
        .map(|member| OperandSpecification::from_holon(member.member))
        .collect())
    }

    /// Classifies this operator by the nearest supported core operator in its lineage.
    ///
    /// Identity decides: an ancestor counts only when it is the canonical
    /// `<Name>.OperatorType` descriptor, so an extension operator executes with
    /// its supported ancestor's semantics and an unrelated same-named descriptor
    /// is unsupported. Contains semantics require an effective case-matching policy.
    pub fn kind(&self) -> Result<OperatorKind, HolonError> {
        let candidates = [
            CoreOperatorTypeName::EqualsOperator,
            CoreOperatorTypeName::LessThanOperator,
            CoreOperatorTypeName::ContainsOperator,
        ]
        .map(|name| (name.as_operator_name().0, name));
        let classified =
            classify_descriptor_lineage(&self.holon, &candidates, operator_type_descriptor_key)?;
        match classified {
            Some(CoreOperatorTypeName::EqualsOperator) => Ok(OperatorKind::Equals),
            Some(CoreOperatorTypeName::LessThanOperator) => Ok(OperatorKind::LessThan),
            Some(CoreOperatorTypeName::ContainsOperator) => match self.case_matching()? {
                Some(case_matching) => Ok(OperatorKind::Contains(case_matching)),
                None => Err(HolonError::InvalidOperatorMetadata {
                    operator: self.operator_name()?.to_string(),
                    reason: "a Contains operator requires an effective CaseMatching".to_string(),
                }),
            },
            None => Err(HolonError::NotImplemented(format!(
                "operator {} has no supported core operator in its lineage",
                self.operator_name()?
            ))),
        }
    }

    /// Resolves this operator against a subject ValueType for repeated invocation.
    pub fn resolve_for_subject(
        &self,
        subject_type: &ValueDescriptor,
    ) -> Result<ResolvedOperator, HolonError> {
        ResolvedOperator::resolve(subject_type, self)
    }

    /// Returns value descriptors that declare this operator as afforded.
    pub fn afforded_by(&self) -> Result<Vec<ValueDescriptor>, HolonError> {
        Ok(effective_relationship_targets(
            &self.holon,
            CoreRelationshipTypeName::ValueTypeAffordedBy,
        )?
        .into_iter()
        .map(|member| ValueDescriptor::from_holon(member.member))
        .collect())
    }
}

impl From<HolonReference> for OperatorDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for OperatorDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<OperatorDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::WritableHolon;
    use base_types::MapEnumValue;

    fn operator_category_value(value: &str) -> MapEnumValue {
        MapEnumValue(MapString(value.to_string()))
    }

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "operator-descriptor",
            "EqualsOperator",
            "Holon",
        )?);

        let descriptor = OperatorDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("EqualsOperator".to_string()));

        Ok(())
    }

    #[test]
    fn accessors_return_declared_values() -> Result<(), HolonError> {
        let context = build_context();
        let value_type =
            new_descriptor_holon(&context, "integer-value-type", "IntegerValueType", "Value")?;
        let mut holon =
            new_descriptor_holon(&context, "equals-operator", "EqualsOperator", "Holon")?;
        holon
            .with_property_value(CorePropertyTypeName::DisplayName, "Equals")?
            .with_property_value(CorePropertyTypeName::Description, "Returns equality.")?
            .with_property_value(CorePropertyTypeName::Arity, 2_i64)?
            .with_property_value(
                CorePropertyTypeName::OperatorCategory,
                operator_category_value("Equality"),
            )?;
        holon.add_related_holons(
            CoreRelationshipTypeName::ValueTypeAffordedBy,
            vec![value_type.clone().into()],
        )?;

        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert_eq!(
            descriptor.operator_name()?,
            OperatorName(MapString("EqualsOperator".to_string()))
        );
        assert_eq!(descriptor.display_name()?, Some(MapString("Equals".to_string())));
        assert_eq!(descriptor.description()?, Some(MapString("Returns equality.".to_string())));
        assert_eq!(descriptor.arity()?, 2);
        assert_eq!(descriptor.operator_category()?, OperatorCategory::Equality);
        assert_eq!(
            descriptor.afforded_by()?[0].header().type_name()?,
            MapString("IntegerValueType".to_string())
        );

        Ok(())
    }

    #[test]
    fn operator_name_uses_shared_type_name() -> Result<(), HolonError> {
        let context = build_context();
        let holon =
            new_descriptor_holon(&context, "less-than-operator", "LessThanOperator", "Holon")?;

        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert_eq!(
            descriptor.operator_name()?,
            OperatorName(MapString("LessThanOperator".to_string()))
        );

        Ok(())
    }

    #[test]
    fn arity_errors_when_required_field_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(&context, "missing-arity", "MissingArity", "Holon")?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.arity(),
            Err(HolonError::EmptyField(field)) if field == "Arity"
        ));

        Ok(())
    }

    #[test]
    fn arity_errors_when_required_field_has_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(&context, "wrong-arity", "WrongArity", "Holon")?;
        holon.with_property_value(CorePropertyTypeName::Arity, "not-an-integer")?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.arity(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Integer"
        ));

        Ok(())
    }

    #[test]
    fn arity_errors_when_integer_is_out_of_range() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(&context, "overflow-arity", "OverflowArity", "Holon")?;
        holon.with_property_value(CorePropertyTypeName::Arity, 256_i64)?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.arity(),
            Err(HolonError::IntegerOutOfRange {
                value,
                min,
                max,
                min_inclusive,
                max_inclusive,
                descriptor
            }) if value == 256
                && min == Some(0)
                && max == Some(255)
                && min_inclusive
                && max_inclusive
                && descriptor == "OperatorDescriptor::arity"
        ));

        Ok(())
    }

    #[test]
    fn operator_category_errors_when_required_field_has_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(
            &context,
            "wrong-operator-category",
            "WrongOperatorCategory",
            "Holon",
        )?;
        holon.with_property_value(CorePropertyTypeName::OperatorCategory, true)?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.operator_category(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Enum"
        ));

        Ok(())
    }

    #[test]
    fn operator_category_accepts_loaded_schema_string_value() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(
            &context,
            "string-backed-operator-category",
            "StringBackedOperatorCategory",
            "Holon",
        )?;
        holon.with_property_value(CorePropertyTypeName::OperatorCategory, "Equality")?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert_eq!(descriptor.operator_category()?, OperatorCategory::Equality);

        Ok(())
    }

    #[test]
    fn operator_category_errors_for_unknown_value() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(
            &context,
            "unknown-operator-category",
            "UnknownOperatorCategory",
            "Holon",
        )?;
        holon.with_property_value(
            CorePropertyTypeName::OperatorCategory,
            operator_category_value("Containment"),
        )?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.operator_category(),
            Err(HolonError::UnknownOperatorCategory { value })
                if value == "Containment"
        ));

        Ok(())
    }

    #[test]
    fn operator_category_rejects_legacy_qualified_variant_name() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(
            &context,
            "qualified-operator-category",
            "QualifiedOperatorCategory",
            "Holon",
        )?;
        holon.with_property_value(
            CorePropertyTypeName::OperatorCategory,
            operator_category_value("OperatorCategory.Equality"),
        )?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.operator_category(),
            Err(HolonError::UnknownOperatorCategory { value })
                if value == "OperatorCategory.Equality"
        ));

        Ok(())
    }
}
