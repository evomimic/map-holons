use std::collections::HashSet;

use crate::descriptors::inheritance::equals_or_extends;
use crate::descriptors::value_descriptor_subtypes::helpers::{
    supported_operators as collect_supported_operators,
    supports_operator as descriptor_supports_operator,
    unsupported_operator as descriptor_unsupported_operator,
    value_kind_mismatch as descriptor_value_kind_mismatch,
};
use crate::descriptors::{
    accessor_helpers, Descriptor, EnumValueDescriptor, IntegerValueDescriptor, OperatorDescriptor,
    StringValueDescriptor, TypeHeader, ValueArrayDescriptor,
};
use crate::reference_layer::HolonReference;
use base_types::{BaseValue, BaseValueKind};
use core_types::HolonError;
use type_names::{CoreOperatorTypeName, ToOperatorName};

/// Runtime wrapper for value-type descriptors.
///
/// `ValueDescriptor` is the public semantic dispatch point for value validation
/// and operator execution. Subtype wrappers own the local behavior for each
/// value kind, while this wrapper resolves the kind through the descriptor
/// inheritance chain and enforces descriptor-level affordances.
pub struct ValueDescriptor {
    holon: HolonReference,
}

impl ValueDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Validates a runtime value against this descriptor's semantic value kind.
    pub fn is_valid(&self, value: &BaseValue) -> Result<(), HolonError> {
        match self.resolved_value_kind()? {
            ValueDescriptorKind::BaseValue(BaseValueKind::Integer) => {
                IntegerValueDescriptor::from_holon(self.holon.clone()).is_valid(value)
            }
            ValueDescriptorKind::BaseValue(BaseValueKind::String) => {
                StringValueDescriptor::from_holon(self.holon.clone()).is_valid(value)
            }
            ValueDescriptorKind::BaseValue(BaseValueKind::Boolean) => self.validate_boolean(value),
            ValueDescriptorKind::BaseValue(BaseValueKind::Enum) => {
                EnumValueDescriptor::from_holon(self.holon.clone()).is_valid(value)
            }
            ValueDescriptorKind::BaseValue(BaseValueKind::Bytes) => self.validate_bytes(value),
            ValueDescriptorKind::AnyBaseValue => Ok(()),
            ValueDescriptorKind::ValueArray => {
                Err(HolonError::NotImplemented("ValueArray validation".to_string()))
            }
            ValueDescriptorKind::Unsupported(found) => Err(self.wrong_value_kind(found)),
        }
    }

    /// Returns operators afforded by this descriptor across its inheritance chain.
    pub fn supported_operators(&self) -> Result<Vec<OperatorDescriptor>, HolonError> {
        collect_supported_operators(&self.holon)
    }

    /// Returns whether this descriptor affords the supplied operator.
    pub fn supports_operator(&self, op: &OperatorDescriptor) -> Result<bool, HolonError> {
        descriptor_supports_operator(&self.holon, op)
    }

    /// Finds an afforded operator by operator descriptor type name.
    pub fn get_operator_by_name<N: ToOperatorName>(
        &self,
        operator_name: N,
    ) -> Result<OperatorDescriptor, HolonError> {
        let requested_name = operator_name.to_operator_name();
        let requested = requested_name.to_string();
        let mut seen = HashSet::new();
        let mut found = None;

        for operator_descriptor in self.supported_operators()? {
            let declaration_name = operator_descriptor.operator_name()?;
            let declaration_label = declaration_name.to_string();
            if !seen.insert(declaration_label.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "operator".to_string(),
                    name: declaration_label,
                    descriptor: accessor_helpers::descriptor_label(&self.holon),
                });
            }
            if declaration_name == requested_name {
                found = Some(operator_descriptor);
            }
        }

        found.ok_or_else(|| HolonError::DescriptorDeclarationNotFound {
            kind: "operator".to_string(),
            name: requested,
            descriptor: accessor_helpers::descriptor_label(&self.holon),
        })
    }

    /// Validates that this descriptor affords the named operator.
    pub fn affords_operator<N: ToOperatorName>(
        &self,
        operator_name: N,
    ) -> Result<OperatorDescriptor, HolonError> {
        self.get_operator_by_name(operator_name)
    }

    /// Returns whether this descriptor affords an operator with the supplied name.
    pub fn supports_operator_by_name<N: ToOperatorName>(
        &self,
        operator_name: N,
    ) -> Result<bool, HolonError> {
        let requested_name = operator_name.to_operator_name();
        for operator_descriptor in self.supported_operators()? {
            if operator_descriptor.operator_name()? == requested_name {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Applies an afforded operator to runtime operands using this descriptor's value kind.
    pub fn apply_operator(
        &self,
        op: &OperatorDescriptor,
        lhs: &BaseValue,
        rhs: &BaseValue,
    ) -> Result<bool, HolonError> {
        let value_kind = self.resolved_value_kind()?;
        if let ValueDescriptorKind::Unsupported(found) = value_kind {
            return Err(self.wrong_value_kind(found));
        }

        if !self.supports_operator(op)? {
            return self.unsupported_operator(op);
        }

        match value_kind {
            ValueDescriptorKind::BaseValue(BaseValueKind::Integer) => {
                IntegerValueDescriptor::from_holon(self.holon.clone()).apply_operator(op, lhs, rhs)
            }
            ValueDescriptorKind::BaseValue(BaseValueKind::String) => {
                StringValueDescriptor::from_holon(self.holon.clone()).apply_operator(op, lhs, rhs)
            }
            ValueDescriptorKind::BaseValue(BaseValueKind::Boolean) => {
                self.apply_boolean_operator(op, lhs, rhs)
            }
            ValueDescriptorKind::BaseValue(BaseValueKind::Enum) => {
                EnumValueDescriptor::from_holon(self.holon.clone()).apply_operator(op, lhs, rhs)
            }
            ValueDescriptorKind::BaseValue(BaseValueKind::Bytes) => {
                self.apply_bytes_operator(op, lhs, rhs)
            }
            ValueDescriptorKind::AnyBaseValue => self.unsupported_operator(op),
            ValueDescriptorKind::ValueArray => {
                // Array execution is explicitly deferred; arrays may expose
                // affordances structurally before they have runtime semantics.
                ValueArrayDescriptor::from_holon(self.holon.clone()).apply_operator(op, lhs, rhs)
            }
            ValueDescriptorKind::Unsupported(_) => {
                unreachable!("Unsupported returns before affordance checks")
            }
        }
    }

    /// Classifies native representation without executing configured constraints.
    ///
    /// Specific native families take precedence over the catch-all base-value
    /// family. Type names are used only in the unsupported-kind diagnostic,
    /// never to select a family.
    /// Reuse the same roots throughout one pass over an unchanged schema snapshot.
    pub fn value_kind(
        &self,
        roots: &super::ResolvedValueTypeRoots,
    ) -> Result<ValueDescriptorKind, HolonError> {
        super::resolved_descriptor_roots::assert_same_transaction(&self.holon, &roots.context)?;
        for (root, kind) in &roots.families {
            if equals_or_extends(&self.holon, root)? {
                return Ok(kind.clone());
            }
        }
        Ok(ValueDescriptorKind::Unsupported(self.header().type_name()?.to_string()))
    }

    /// Existing one-off operations resolve through their already-bound transaction.
    fn resolved_value_kind(&self) -> Result<ValueDescriptorKind, HolonError> {
        self.value_kind(&super::ResolvedValueTypeRoots::resolve(&self.holon.bound_context())?)
    }

    fn validate_boolean(&self, value: &BaseValue) -> Result<(), HolonError> {
        match value.kind() {
            BaseValueKind::Boolean => Ok(()),
            BaseValueKind::String
            | BaseValueKind::Integer
            | BaseValueKind::Enum
            | BaseValueKind::Bytes => Err(self.value_kind_mismatch("Boolean", value)),
        }
    }

    fn validate_bytes(&self, value: &BaseValue) -> Result<(), HolonError> {
        match value.kind() {
            BaseValueKind::Bytes => Ok(()),
            BaseValueKind::String
            | BaseValueKind::Boolean
            | BaseValueKind::Integer
            | BaseValueKind::Enum => Err(self.value_kind_mismatch("Bytes", value)),
        }
    }

    fn apply_boolean_operator(
        &self,
        op: &OperatorDescriptor,
        lhs: &BaseValue,
        rhs: &BaseValue,
    ) -> Result<bool, HolonError> {
        if op.operator_name()? != CoreOperatorTypeName::EqualsOperator.as_operator_name() {
            return self.unsupported_operator(op);
        }

        let lhs = match lhs {
            BaseValue::BooleanValue(value) => value,
            other => return Err(self.value_kind_mismatch("Boolean", other)),
        };
        let rhs = match rhs {
            BaseValue::BooleanValue(value) => value,
            other => return Err(self.value_kind_mismatch("Boolean", other)),
        };
        Ok(lhs == rhs)
    }

    fn apply_bytes_operator(
        &self,
        op: &OperatorDescriptor,
        lhs: &BaseValue,
        rhs: &BaseValue,
    ) -> Result<bool, HolonError> {
        if op.operator_name()? != CoreOperatorTypeName::EqualsOperator.as_operator_name() {
            return self.unsupported_operator(op);
        }

        let lhs = match lhs {
            BaseValue::BytesValue(value) => value,
            other => return Err(self.value_kind_mismatch("Bytes", other)),
        };
        let rhs = match rhs {
            BaseValue::BytesValue(value) => value,
            other => return Err(self.value_kind_mismatch("Bytes", other)),
        };
        Ok(lhs == rhs)
    }

    fn unsupported_operator(&self, op: &OperatorDescriptor) -> Result<bool, HolonError> {
        descriptor_unsupported_operator(&self.holon, op)
    }

    fn value_kind_mismatch(&self, expected: &str, found: &BaseValue) -> HolonError {
        descriptor_value_kind_mismatch(&self.holon, expected, found)
    }

    fn wrong_value_kind(&self, found: String) -> HolonError {
        HolonError::WrongDescriptorKind {
            expected: "IntegerValueType, StringValueType, BooleanValueType, BytesValueType, EnumValueType, BaseValueValueType, or ValueArrayValueType".to_string(),
            found,
            descriptor: accessor_helpers::descriptor_label(&self.holon),
        }
    }
}

/// Native representation classified independently of configured constraints.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ValueDescriptorKind {
    /// One of the five scalar representations supported by `BaseValue`.
    BaseValue(BaseValueKind),
    /// Accepts any native `BaseValue` representation.
    AnyBaseValue,
    /// Array representation, whose execution semantics remain deferred.
    ValueArray,
    /// No supported family root occurs in the lineage; carries a diagnostic label.
    Unsupported(String),
}

impl From<HolonReference> for ValueDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for ValueDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<ValueDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::transactions::TransactionContext;
    use crate::descriptors::test_support;
    use crate::reference_layer::TransientReference;
    use crate::reference_layer::WritableHolon;
    use base_types::{MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};
    use core_types::HolonError;
    use std::sync::Arc;
    use type_names::CoreRelationshipTypeName;

    // Existing dispatch fixtures now extend actual canonical identities. A matching
    // TypeName alone must no longer grant native representation semantics.
    fn build_context() -> Arc<TransactionContext> {
        let context = test_support::build_context();
        for name in [
            "StringValueType",
            "IntegerValueType",
            "BooleanValueType",
            "BytesValueType",
            "EnumValueType",
            "BaseValueValueType",
            "ValueArrayValueType",
        ] {
            let root = test_support::new_descriptor_holon(
                &context,
                &format!("{name}.ValueType"),
                name,
                "Value",
            )
            .unwrap();
            context.mutation().stage_new_holon(root).unwrap();
        }
        context
    }

    fn new_descriptor_holon(
        context: &Arc<TransactionContext>,
        key: &str,
        type_name: &str,
        kind: &str,
    ) -> Result<TransientReference, HolonError> {
        let mut holon = test_support::new_descriptor_holon(context, key, type_name, kind)?;
        if let Ok(root) = context
            .lookup()
            .get_staged_holon_by_base_key(&MapString(format!("{type_name}.ValueType")))
        {
            holon.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        }
        Ok(holon)
    }

    #[test]
    fn value_kind_uses_family_identity_for_every_representation() -> Result<(), HolonError> {
        let context = build_context();
        let roots = super::super::ResolvedValueTypeRoots::resolve(&context)?;
        for (root, expected) in &roots.families {
            assert_eq!(ValueDescriptor::from_holon(root.clone()).value_kind(&roots)?, *expected);

            let mut child = test_support::new_descriptor_holon(
                &context,
                &format!("child-{}", root.reference_id_string()),
                "UnrelatedDisplayName",
                "Value",
            )?;
            child.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.clone()])?;
            assert_eq!(ValueDescriptor::from_holon(child.into()).value_kind(&roots)?, *expected);
        }

        // A same-named impostor has no relationship to the resolved canonical root.
        let impostor =
            test_support::new_descriptor_holon(&context, "impostor", "StringValueType", "Value")?;
        assert_eq!(
            ValueDescriptor::from_holon(impostor.into()).value_kind(&roots)?,
            ValueDescriptorKind::Unsupported("StringValueType".into())
        );
        Ok(())
    }

    #[test]
    fn value_kind_is_independent_of_enum_membership_and_constraints() -> Result<(), HolonError> {
        let context = build_context();
        let roots = super::super::ResolvedValueTypeRoots::resolve(&context)?;
        let mut value =
            new_descriptor_holon(&context, "enum-without-variants", "EnumValueType", "Value")?;
        let constraint = test_support::new_test_holon(&context, "unsupported-constraint")?;
        value.add_related_holons(CoreRelationshipTypeName::Constraints, vec![constraint.into()])?;
        assert_eq!(
            ValueDescriptor::from_holon(value.into()).value_kind(&roots)?,
            ValueDescriptorKind::BaseValue(BaseValueKind::Enum)
        );
        Ok(())
    }

    #[test]
    fn value_kind_rejects_foreign_transactions_and_broken_lineages() -> Result<(), HolonError> {
        let context = build_context();
        let roots = super::super::ResolvedValueTypeRoots::resolve(&context)?;
        let foreign_context = build_context();
        let foreign =
            new_descriptor_holon(&foreign_context, "foreign", "StringValueType", "Value")?;
        assert!(matches!(
            ValueDescriptor::from_holon(foreign.into()).value_kind(&roots),
            Err(HolonError::CrossTransactionReference { .. })
        ));
        let mut cycle = test_support::new_descriptor_holon(&context, "cycle", "Cycle", "Value")?;
        cycle.add_related_holons(CoreRelationshipTypeName::Extends, vec![(&cycle).into()])?;
        assert!(matches!(
            ValueDescriptor::from_holon(cycle.into()).value_kind(&roots),
            Err(HolonError::CyclicExtends { .. })
        ));
        Ok(())
    }

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "value-descriptor",
            "StringValueType",
            "Value",
        )?);

        let descriptor = ValueDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("StringValueType".to_string()));

        Ok(())
    }

    #[test]
    fn is_valid_routes_by_value_kind() -> Result<(), HolonError> {
        let context = build_context();
        let integer = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?.into(),
        );

        assert!(integer.is_valid(&BaseValue::IntegerValue(MapInteger(42))).is_ok());
        assert!(matches!(
            integer.is_valid(&BaseValue::StringValue(MapString("42".to_string()))),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Integer" && found == "String"
        ));

        Ok(())
    }

    #[test]
    fn is_valid_handles_boolean_inline() -> Result<(), HolonError> {
        let context = build_context();
        let boolean = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "boolean-value", "BooleanValueType", "Value")?.into(),
        );

        assert!(boolean.is_valid(&BaseValue::BooleanValue(MapBoolean(true))).is_ok());
        assert!(matches!(
            boolean.is_valid(&BaseValue::IntegerValue(MapInteger(1))),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Boolean" && found == "Integer"
        ));

        Ok(())
    }

    #[test]
    fn is_valid_handles_bytes_inline() -> Result<(), HolonError> {
        let context = build_context();
        let bytes = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "bytes-value", "BytesValueType", "Value")?.into(),
        );

        assert!(bytes.is_valid(&BaseValue::BytesValue(MapBytes(vec![1, 2, 3]))).is_ok());
        assert!(matches!(
            bytes.is_valid(&BaseValue::StringValue(MapString("010203".to_string()))),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Bytes" && found == "String"
        ));

        Ok(())
    }

    #[test]
    fn base_value_value_type_accepts_every_base_value_kind() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "base-value-type", "BaseValueValueType", "Value")?
                .into(),
        );
        let values = [
            BaseValue::StringValue(MapString("value".to_string())),
            BaseValue::BooleanValue(MapBoolean(true)),
            BaseValue::IntegerValue(MapInteger(42)),
            BaseValue::EnumValue(MapEnumValue(MapString("Member".to_string()))),
            BaseValue::BytesValue(MapBytes(vec![1, 2, 3])),
        ];

        for value in values {
            descriptor.is_valid(&value)?;
        }

        Ok(())
    }

    #[test]
    fn value_array_validation_is_explicitly_deferred() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "value-array-type", "ValueArrayValueType", "Value")?
                .into(),
        );

        assert!(matches!(
            descriptor.is_valid(&BaseValue::StringValue(MapString("item".to_string()))),
            Err(HolonError::NotImplemented(message)) if message == "ValueArray validation"
        ));

        Ok(())
    }

    #[test]
    fn is_valid_resolves_kind_through_extends_chain() -> Result<(), HolonError> {
        let context = build_context();
        let parent = new_descriptor_holon(&context, "integer-parent", "IntegerValueType", "Value")?;
        let mut child =
            new_descriptor_holon(&context, "integer-child", "CustomIntegerValueType", "Value")?;
        child.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;

        let descriptor = ValueDescriptor::from_holon(child.into());

        assert!(descriptor.is_valid(&BaseValue::IntegerValue(MapInteger(42))).is_ok());

        Ok(())
    }

    #[test]
    fn is_valid_reports_wrong_descriptor_kind_for_unknown_value_kind() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "unknown-value", "CustomValueType", "Value")?.into(),
        );

        assert!(matches!(
            descriptor.is_valid(&BaseValue::IntegerValue(MapInteger(1))),
            Err(HolonError::WrongDescriptorKind { found, .. }) if found == "CustomValueType"
        ));

        Ok(())
    }

    #[test]
    fn supported_operators_flattens_across_extends() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut parent =
            new_descriptor_holon(&context, "integer-parent", "IntegerValueType", "Value")?;
        let mut child =
            new_descriptor_holon(&context, "integer-child", "CustomIntegerValueType", "Value")?;
        parent.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;
        child.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;

        let descriptor = ValueDescriptor::from_holon(child.into());
        let names = descriptor
            .supported_operators()?
            .into_iter()
            .map(|op| op.operator_name().map(|name| name.0.to_string()))
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(names, vec!["EqualsOperator"]);

        Ok(())
    }

    #[test]
    fn get_operator_by_name_returns_match_and_reports_missing() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?;
        value.add_related_holons(CoreRelationshipTypeName::AffordsOperator, vec![equals.into()])?;

        let descriptor = ValueDescriptor::from_holon(value.into());

        assert_eq!(
            descriptor.get_operator_by_name("equals_operator")?.operator_name()?.0,
            MapString("EqualsOperator".to_string())
        );
        assert!(matches!(
            descriptor.get_operator_by_name("less_than_operator"),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "operator" && name == "LessThanOperator"
        ));

        Ok(())
    }

    #[test]
    fn affords_operator_delegates_to_operator_lookup() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "afforded-equals", "EqualsOperator", "Holon")?;
        let mut value = new_descriptor_holon(
            &context,
            "operator-validation-value",
            "IntegerValueType",
            "Value",
        )?;
        value.add_related_holons(CoreRelationshipTypeName::AffordsOperator, vec![equals.into()])?;

        let descriptor = ValueDescriptor::from_holon(value.into());

        assert_eq!(
            descriptor.affords_operator("equals_operator")?.operator_name()?.0,
            MapString("EqualsOperator".to_string())
        );
        assert!(matches!(
            descriptor.affords_operator("less_than_operator"),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "operator" && name == "LessThanOperator"
        ));

        Ok(())
    }

    #[test]
    fn affords_operator_preserves_duplicate_inherited_declaration_errors() -> Result<(), HolonError>
    {
        let context = build_context();
        let duplicate_parent =
            new_descriptor_holon(&context, "affords-parent-equals", "EqualsOperator", "Holon")?;
        let duplicate_child =
            new_descriptor_holon(&context, "affords-child-equals", "EqualsOperator", "Holon")?;
        let mut parent =
            new_descriptor_holon(&context, "affords-integer-parent", "IntegerValueType", "Value")?;
        let mut child = new_descriptor_holon(
            &context,
            "affords-integer-child",
            "CustomIntegerValueType",
            "Value",
        )?;

        parent.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![duplicate_parent.into()],
        )?;
        child.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;
        child.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![duplicate_child.into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(child.into());

        assert!(matches!(
            descriptor.affords_operator("equals_operator"),
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "operator" && name == "EqualsOperator"
        ));

        Ok(())
    }

    #[test]
    fn get_operator_by_name_detects_duplicate_inherited_declarations() -> Result<(), HolonError> {
        let context = build_context();
        let duplicate_parent =
            new_descriptor_holon(&context, "parent-equals", "EqualsOperator", "Holon")?;
        let duplicate_child =
            new_descriptor_holon(&context, "child-equals", "EqualsOperator", "Holon")?;
        let mut parent =
            new_descriptor_holon(&context, "integer-parent", "IntegerValueType", "Value")?;
        let mut child =
            new_descriptor_holon(&context, "integer-child", "CustomIntegerValueType", "Value")?;

        parent.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![duplicate_parent.into()],
        )?;
        child.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;
        child.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![duplicate_child.into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(child.into());

        assert!(matches!(
            descriptor.get_operator_by_name("equals_operator"),
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "operator" && name == "EqualsOperator"
        ));

        Ok(())
    }

    #[test]
    fn supports_operator_by_name_reports_membership() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?;
        value.add_related_holons(CoreRelationshipTypeName::AffordsOperator, vec![equals.into()])?;

        let descriptor = ValueDescriptor::from_holon(value.into());

        assert!(descriptor.supports_operator_by_name("equals_operator")?);
        assert!(!descriptor.supports_operator_by_name("less_than_operator")?);

        Ok(())
    }

    #[test]
    fn supports_operator_reports_membership() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let less_than = new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(value.into());

        assert!(descriptor.supports_operator(&OperatorDescriptor::from_holon(equals.into()))?);
        assert!(!descriptor.supports_operator(&OperatorDescriptor::from_holon(less_than.into()))?);

        Ok(())
    }

    #[test]
    fn apply_operator_executes_afforded_integer_operator() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(value.into());
        let equals = OperatorDescriptor::from_holon(equals.into());

        assert!(descriptor.apply_operator(
            &equals,
            &BaseValue::IntegerValue(MapInteger(3)),
            &BaseValue::IntegerValue(MapInteger(3)),
        )?);

        Ok(())
    }

    #[test]
    fn apply_operator_returns_unsupported_when_operator_is_not_afforded() -> Result<(), HolonError>
    {
        let context = build_context();
        let equals = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?.into(),
        );
        let descriptor = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?.into(),
        );

        assert!(matches!(
            descriptor.apply_operator(
                &equals,
                &BaseValue::IntegerValue(MapInteger(3)),
                &BaseValue::IntegerValue(MapInteger(3)),
            ),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "EqualsOperator" && value_type == "IntegerValueType"
        ));

        Ok(())
    }

    #[test]
    fn apply_operator_handles_boolean_equals_inline() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "boolean-value", "BooleanValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(value.into());
        let equals = OperatorDescriptor::from_holon(equals.into());

        assert!(descriptor.apply_operator(
            &equals,
            &BaseValue::BooleanValue(MapBoolean(true)),
            &BaseValue::BooleanValue(MapBoolean(true)),
        )?);

        Ok(())
    }

    #[test]
    fn apply_operator_reports_wrong_descriptor_kind_for_unknown_value_kind(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "unknown-value", "CustomValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(value.into());
        let equals = OperatorDescriptor::from_holon(equals.into());

        assert!(matches!(
            descriptor.apply_operator(
                &equals,
                &BaseValue::IntegerValue(MapInteger(3)),
                &BaseValue::IntegerValue(MapInteger(3)),
            ),
            Err(HolonError::WrongDescriptorKind { found, .. }) if found == "CustomValueType"
        ));

        Ok(())
    }

    #[test]
    fn apply_operator_reports_unsupported_for_afforded_boolean_non_equals_operator(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let less_than = new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "boolean-value", "BooleanValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![less_than.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(value.into());
        let less_than = OperatorDescriptor::from_holon(less_than.into());

        assert!(matches!(
            descriptor.apply_operator(
                &less_than,
                &BaseValue::BooleanValue(MapBoolean(false)),
                &BaseValue::BooleanValue(MapBoolean(true)),
            ),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "LessThanOperator" && value_type == "BooleanValueType"
        ));

        Ok(())
    }

    #[test]
    fn apply_operator_dispatches_enum_values() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let parent = new_descriptor_holon(&context, "enum-parent", "EnumValueType", "Value")?;
        let red = new_descriptor_holon(&context, "red", "Red", "EnumVariant")?;
        let blue = new_descriptor_holon(&context, "blue", "Blue", "EnumVariant")?;
        let mut color = new_descriptor_holon(&context, "color", "ColorValueType", "Value")?;
        color.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;
        color.add_related_holons(
            CoreRelationshipTypeName::Variants,
            vec![red.into(), blue.into()],
        )?;
        color.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(color.into());
        let equals = OperatorDescriptor::from_holon(equals.into());
        let red = BaseValue::EnumValue(MapEnumValue(MapString("Red".to_string())));
        let blue = BaseValue::EnumValue(MapEnumValue(MapString("Blue".to_string())));

        assert!(descriptor.is_valid(&red).is_ok());
        assert!(descriptor.apply_operator(&equals, &red, &red)?);
        assert!(!descriptor.apply_operator(&equals, &red, &blue)?);

        Ok(())
    }

    #[test]
    fn apply_operator_dispatches_array_to_deferred_execution() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut array = new_descriptor_holon(&context, "array", "ValueArrayValueType", "Value")?;
        array.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(array.into());
        let equals = OperatorDescriptor::from_holon(equals.into());

        assert!(matches!(
            descriptor.apply_operator(
                &equals,
                &BaseValue::IntegerValue(MapInteger(1)),
                &BaseValue::IntegerValue(MapInteger(1)),
            ),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "EqualsOperator" && value_type == "ValueArrayValueType"
        ));

        Ok(())
    }
}
