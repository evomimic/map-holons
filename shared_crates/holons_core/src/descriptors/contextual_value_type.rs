//! Holder-dependent value contracts shared by validation and presentation.
use super::{core_roles, Descriptor, PropertyDescriptor, ValueDescriptor};
use crate::HolonReference;
use core_types::HolonError;
use type_names::CoreDefinitionRole;

/// Descriptor facts may come from live references or an assessment's prepared diagnoses.
/// A missing/invalid selected type is distinct from an operational read failure.
pub trait ContextualValueTypeFacts {
    /// Classify a definition using the caller's live lineage or prepared structural diagnosis.
    fn has_role(
        &self,
        reference: &HolonReference,
        role: CoreDefinitionRole,
    ) -> Result<bool, HolonError>;
    /// Read the local selection; None denotes a structurally unavailable selection.
    fn selected_value_type(
        &self,
        property: &HolonReference,
    ) -> Result<Option<HolonReference>, HolonError>;
}

/// The exact schema definitions governing a bound field, without reading its value.
#[derive(Debug, Clone)]
pub struct ContextualValueType {
    /// The field descriptor's local representation contract.
    pub declared: HolonReference,
    /// The exact type governing this bound occurrence.
    pub contextual: HolonReference,
    /// The property definition selecting the contextual type; also supplies provenance.
    pub governing_property: HolonReference,
    /// Holder metadata contributes to this contract and must participate in reuse.
    pub holder_dependent: bool,
}

/// Resolve the existing dependent-default semantics once, independently of acquisition.
/// The canonical field is identified by its definition key, never its display name.
pub fn resolve_contextual_value_type(
    holder: &HolonReference,
    property: &HolonReference,
    facts: &impl ContextualValueTypeFacts,
) -> Result<Option<ContextualValueType>, HolonError> {
    let holder_dependent =
        core_roles::record_key(property)?.is_some_and(|key| key.0 == "DefaultValue.PropertyType");
    let governing_property = if holder_dependent {
        if !facts.has_role(property, CoreDefinitionRole::PropertyType)?
            || !facts.has_role(holder, CoreDefinitionRole::PropertyType)?
        {
            return Ok(None);
        }
        holder
    } else {
        property
    };
    let Some(declared) = facts.selected_value_type(property)? else { return Ok(None) };
    let contextual = if governing_property == property {
        declared.clone()
    } else {
        let Some(target) = resolve_default_value_type(governing_property, facts)? else {
            return Ok(None);
        };
        target
    };
    if !facts.has_role(&declared, CoreDefinitionRole::ValueType)?
        || !facts.has_role(&contextual, CoreDefinitionRole::ValueType)?
    {
        return Ok(None);
    }
    Ok(Some(ContextualValueType {
        declared,
        contextual,
        governing_property: governing_property.clone(),
        holder_dependent,
    }))
}

/// The DS-DEFAULT-002 target, available independently of field presence and runtime schema assembly.
/// Validation invokes this same semantic product using its prepared facts.
pub fn resolve_default_value_type(
    property: &HolonReference,
    facts: &impl ContextualValueTypeFacts,
) -> Result<Option<HolonReference>, HolonError> {
    let Some(target) = facts.selected_value_type(property)? else { return Ok(None) };
    Ok(facts.has_role(&target, CoreDefinitionRole::ValueType)?.then_some(target))
}

pub(super) struct LiveValueTypeFacts;
impl ContextualValueTypeFacts for LiveValueTypeFacts {
    fn has_role(
        &self,
        reference: &HolonReference,
        role: CoreDefinitionRole,
    ) -> Result<bool, HolonError> {
        core_roles::extends_role(reference, role)
    }
    fn selected_value_type(
        &self,
        property: &HolonReference,
    ) -> Result<Option<HolonReference>, HolonError> {
        Ok(Some(PropertyDescriptor::from_holon(property.clone()).value_type()?.holon().clone()))
    }
}

impl PropertyDescriptor {
    /// Resolve this field's type in its holder's context, without reading or defaulting the field.
    pub fn contextual_value_type(
        &self,
        holder: &HolonReference,
    ) -> Result<ContextualValueType, HolonError> {
        resolve_contextual_value_type(holder, self.holon(), &LiveValueTypeFacts)?.ok_or_else(|| {
            HolonError::InvalidState(
                "Property occurrence requires one usable contextual ValueType".into(),
            )
        })
    }
}

impl ContextualValueType {
    /// Wrap the resolved type for ordinary kind and effective-constraint access.
    pub fn value_descriptor(&self) -> ValueDescriptor {
        ValueDescriptor::from_holon(self.contextual.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_test_holon};
    use crate::{ReadableHolon, TransientReference, WritableHolon};
    use type_names::CoreRelationshipTypeName as R;

    #[test]
    fn contextual_defaults_use_the_holder_without_reading_or_materializing_values(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let property_root: HolonReference =
            new_test_holon(&context, "PropertyType.TypeDescriptor")?.into();
        let value_root: HolonReference =
            new_test_holon(&context, "ValueType.TypeDescriptor")?.into();
        let make_property =
            |key: &str, target: HolonReference| -> Result<TransientReference, HolonError> {
                let mut reference = new_test_holon(&context, key)?;
                reference.add_related_holons(R::Extends, vec![property_root.clone()])?;
                reference.add_related_holons(R::ValueType, vec![target])?;
                Ok(reference)
            };
        let broad: HolonReference =
            new_test_holon(&context, "BaseValueValueType.ValueType")?.into();
        let mut broad = broad;
        broad.add_related_holons(R::Extends, vec![value_root.clone()])?;
        let mut boolean: HolonReference =
            new_test_holon(&context, "MapBooleanValueType.BooleanValueType")?.into();
        boolean.add_related_holons(R::Extends, vec![value_root.clone()])?;
        let mut string: HolonReference =
            new_test_holon(&context, "MapStringValueType.StringValueType")?.into();
        string.add_related_holons(R::Extends, vec![value_root])?;
        let default: HolonReference =
            make_property("DefaultValue.PropertyType", broad.clone())?.into();
        let mut holder: HolonReference =
            make_property("BooleanFlag.PropertyType", boolean.clone())?.into();
        let other: HolonReference = make_property("Title.PropertyType", string.clone())?.into();
        let property = PropertyDescriptor::from_holon(default.clone());
        let absent = property.contextual_value_type(&holder)?;
        assert_eq!(absent.declared, broad);
        assert_eq!(absent.contextual, boolean);
        assert_eq!(absent.governing_property, holder);
        assert!(holder.property_value("DefaultValue")?.is_none());
        holder.with_property_value("DefaultValue", true)?;
        assert_eq!(property.contextual_value_type(&holder)?.contextual, boolean);
        assert_eq!(property.contextual_value_type(&other)?.contextual, string);
        let unrelated: HolonReference =
            make_property("Unrelated.PropertyType", broad.clone())?.into();
        let mut unrelated = unrelated;
        unrelated.with_property_value("TypeName", "DefaultValue")?;
        assert_eq!(
            PropertyDescriptor::from_holon(unrelated).contextual_value_type(&holder)?.contextual,
            broad
        );
        assert_eq!(
            resolve_default_value_type(&holder, &LiveValueTypeFacts)?,
            Some(boolean.clone())
        );
        holder.remove_related_holons(R::ValueType, vec![boolean])?;
        assert!(
            property.contextual_value_type(&holder).is_err(),
            "missing holder type cannot fall back to BaseValue"
        );
        let wrong = new_test_holon(&context, "not-a-value-type")?;
        holder.add_related_holons(R::ValueType, vec![wrong.into()])?;
        assert!(
            property.contextual_value_type(&holder).is_err(),
            "unusable holder type cannot fall back to BaseValue"
        );
        Ok(())
    }

    #[test]
    fn prepared_resolution_keeps_missing_unusable_and_operational_failures_distinct(
    ) -> Result<(), HolonError> {
        struct Facts {
            target: Result<Option<HolonReference>, HolonError>,
            usable: bool,
        }
        impl ContextualValueTypeFacts for Facts {
            fn has_role(
                &self,
                _: &HolonReference,
                _: CoreDefinitionRole,
            ) -> Result<bool, HolonError> {
                Ok(self.usable)
            }
            fn selected_value_type(
                &self,
                _: &HolonReference,
            ) -> Result<Option<HolonReference>, HolonError> {
                self.target.clone()
            }
        }
        let context = build_context();
        let property: HolonReference = new_test_holon(&context, "property")?.into();
        let target: HolonReference = new_test_holon(&context, "target")?.into();
        assert_eq!(
            resolve_default_value_type(&property, &Facts { target: Ok(None), usable: true })?,
            None
        );
        assert_eq!(
            resolve_default_value_type(
                &property,
                &Facts { target: Ok(Some(target)), usable: false }
            )?,
            None
        );
        let failure = HolonError::NotAccessible("ValueType".into(), "backend unavailable".into());
        assert_eq!(
            resolve_default_value_type(
                &property,
                &Facts { target: Err(failure.clone()), usable: true }
            ),
            Err(failure)
        );
        Ok(())
    }
}
