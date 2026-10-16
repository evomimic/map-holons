use crate::descriptors::{accessor_helpers, ValueDescriptor};
use crate::reference_layer::HolonReference;
use base_types::MapString;
use core_types::HolonError;
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

/// How an operand specification resolves its required operand ValueType.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OperandValueTypeRule {
    /// The exact resolved subject ValueType, retaining its effective constraints.
    SameAsSubject,
    /// The ValueType referenced by the specification's `OperandValueType`.
    FixedValueType,
}

impl OperandValueTypeRule {
    /// Parses the enum variant TypeName stored on operand specifications.
    pub fn parse(value: &MapString) -> Result<Self, HolonError> {
        match value.0.as_str() {
            "SameAsSubject" => Ok(Self::SameAsSubject),
            "FixedValueType" => Ok(Self::FixedValueType),
            _ => Err(HolonError::UnknownOperandValueTypeRule { value: value.to_string() }),
        }
    }
}

/// Read view over an operator's operand specification holon.
///
/// Specifications are ordinary holons, not descriptors: each describes one
/// authored operand position after the subject. Position comes from the
/// operator's ordered `OperandSpecifications` relationship.
#[derive(Debug, Clone)]
pub struct OperandSpecification {
    holon: HolonReference,
}

impl OperandSpecification {
    /// Wraps an already-resolved operand specification holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Returns the underlying specification holon.
    pub fn holon(&self) -> &HolonReference {
        &self.holon
    }

    /// Returns the durable role name of this operand position.
    pub fn role(&self) -> Result<MapString, HolonError> {
        accessor_helpers::require_string(&self.holon, CorePropertyTypeName::OperandRole)
    }

    /// Returns the human-facing label of this operand position.
    pub fn display_name(&self) -> Result<MapString, HolonError> {
        accessor_helpers::require_string(&self.holon, CorePropertyTypeName::DisplayName)
    }

    /// Returns the optional human-facing description of this operand position.
    pub fn description(&self) -> Result<Option<MapString>, HolonError> {
        accessor_helpers::optional_string(&self.holon, CorePropertyTypeName::Description)
    }

    /// Returns the rule resolving this operand's required ValueType.
    pub fn rule(&self) -> Result<OperandValueTypeRule, HolonError> {
        let value = accessor_helpers::require_enum_string(
            &self.holon,
            CorePropertyTypeName::OperandValueTypeRule,
        )?;
        OperandValueTypeRule::parse(&value)
    }

    /// Returns the declared fixed operand ValueType, if any.
    pub fn operand_value_type(&self) -> Result<Option<ValueDescriptor>, HolonError> {
        Ok(accessor_helpers::optional_single_related(
            &self.holon,
            CoreRelationshipTypeName::OperandValueType,
        )?
        .map(ValueDescriptor::from_holon))
    }

    /// Resolves the ValueType an operand at this position must have for `subject_type`.
    ///
    /// `FixedValueType` requires exactly one `OperandValueType`; `SameAsSubject`
    /// forbids it and yields the subject's own ValueType, constraints included.
    pub fn required_value_type(
        &self,
        operator: &str,
        subject_type: &ValueDescriptor,
    ) -> Result<ValueDescriptor, HolonError> {
        match (self.rule()?, self.operand_value_type()?) {
            (OperandValueTypeRule::SameAsSubject, None) => Ok(subject_type.clone()),
            (OperandValueTypeRule::FixedValueType, Some(value_type)) => Ok(value_type),
            (OperandValueTypeRule::SameAsSubject, Some(_)) => Err(self.invalid(
                operator,
                "a SameAsSubject operand specification must not declare OperandValueType",
            )),
            (OperandValueTypeRule::FixedValueType, None) => Err(self.invalid(
                operator,
                "a FixedValueType operand specification requires one OperandValueType",
            )),
        }
    }

    fn invalid(&self, operator: &str, reason: &str) -> HolonError {
        HolonError::InvalidOperatorMetadata {
            operator: operator.to_string(),
            reason: format!("{reason} ({})", accessor_helpers::descriptor_label(&self.holon)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon, new_test_holon};
    use crate::descriptors::Descriptor;
    use crate::reference_layer::{TransientReference, WritableHolon};
    use base_types::MapEnumValue;

    fn specification(
        context: &std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
        rule: &str,
        operand_value_type: Option<HolonReference>,
    ) -> OperandSpecification {
        let mut holon: TransientReference = new_test_holon(context, "spec").unwrap();
        holon
            .with_property_value(CorePropertyTypeName::OperandRole, "Substring")
            .unwrap()
            .with_property_value(CorePropertyTypeName::DisplayName, "Substring")
            .unwrap()
            .with_property_value(
                CorePropertyTypeName::OperandValueTypeRule,
                MapEnumValue(MapString(rule.to_string())),
            )
            .unwrap();
        if let Some(value_type) = operand_value_type {
            holon
                .add_related_holons(CoreRelationshipTypeName::OperandValueType, vec![value_type])
                .unwrap();
        }
        OperandSpecification::from_holon(holon.into())
    }

    fn value_type(
        context: &std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
        name: &str,
    ) -> ValueDescriptor {
        ValueDescriptor::from_holon(
            new_descriptor_holon(context, &format!("{name}.ValueType"), name, "Value")
                .unwrap()
                .into(),
        )
    }

    #[test]
    fn accessors_read_declared_metadata() {
        let context = build_context();
        let spec = specification(&context, "SameAsSubject", None);
        assert_eq!(spec.role().unwrap().0, "Substring");
        assert_eq!(spec.display_name().unwrap().0, "Substring");
        assert_eq!(spec.description().unwrap(), None);
        assert_eq!(spec.rule().unwrap(), OperandValueTypeRule::SameAsSubject);
    }

    #[test]
    fn same_as_subject_yields_the_subject_type() {
        let context = build_context();
        let subject = value_type(&context, "Title");
        let spec = specification(&context, "SameAsSubject", None);
        let required = spec.required_value_type("EqualsOperator", &subject).unwrap();
        assert_eq!(required.holon(), subject.holon());
    }

    #[test]
    fn fixed_value_type_yields_its_declared_type() {
        let context = build_context();
        let subject = value_type(&context, "Title");
        let fixed = value_type(&context, "MapStringValueType");
        let spec = specification(&context, "FixedValueType", Some(fixed.holon().clone()));
        let required = spec.required_value_type("ContainsOperator", &subject).unwrap();
        assert_eq!(required.holon(), fixed.holon());
    }

    #[test]
    fn rule_and_relationship_must_agree() {
        let context = build_context();
        let subject = value_type(&context, "Title");
        let fixed = value_type(&context, "MapStringValueType");

        let same_with_type = specification(&context, "SameAsSubject", Some(fixed.holon().clone()));
        assert!(matches!(
            same_with_type.required_value_type("EqualsOperator", &subject),
            Err(HolonError::InvalidOperatorMetadata { operator, .. }) if operator == "EqualsOperator"
        ));

        let fixed_without_type = specification(&context, "FixedValueType", None);
        assert!(matches!(
            fixed_without_type.required_value_type("ContainsOperator", &subject),
            Err(HolonError::InvalidOperatorMetadata { operator, .. })
                if operator == "ContainsOperator"
        ));
    }

    #[test]
    fn unknown_rule_is_rejected() {
        let context = build_context();
        let spec = specification(&context, "SubjectFamily", None);
        assert!(matches!(
            spec.rule(),
            Err(HolonError::UnknownOperandValueTypeRule { value }) if value == "SubjectFamily"
        ));
    }
}
