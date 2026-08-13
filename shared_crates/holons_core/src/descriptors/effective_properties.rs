//! Policy-aware effective instance-property resolution.

use std::collections::HashMap;

use crate::descriptors::{
    accessor_helpers, walk_extends_chain, Descriptor, HolonDescriptor, PropertyDescriptor,
};
use crate::reference_layer::ReadableHolon;
use base_types::{BaseValue, MapString};
use core_types::HolonError;
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

#[derive(Clone, Copy)]
enum InheritanceMode {
    None,
    Additive,
    Override,
}

/// Returns the effective `InstanceProperties` declarations for `descriptor`.
///
/// The relationship descriptor that licenses `InstanceProperties` supplies the
/// inheritance mode. This deliberately does not assume Core's current
/// `Additive` setting.
pub(crate) fn effective_instance_properties(
    descriptor: &HolonDescriptor,
) -> Result<Vec<PropertyDescriptor>, HolonError> {
    let mode = instance_properties_inheritance_mode(descriptor)?;
    let members = match mode {
        InheritanceMode::None => local_instance_properties(descriptor.holon())?,
        InheritanceMode::Additive => additive_instance_properties(descriptor.holon())?,
        InheritanceMode::Override => override_instance_properties(descriptor.holon())?,
    };

    normalize_property_members(descriptor, members)
}

fn instance_properties_inheritance_mode(
    descriptor: &HolonDescriptor,
) -> Result<InheritanceMode, HolonError> {
    let meta_descriptor = descriptor.holon().holon_descriptor()?;
    let relationship =
        meta_descriptor.get_relationship_by_name(CoreRelationshipTypeName::InstanceProperties)?;
    let value = relationship
        .holon()
        .property_value(CorePropertyTypeName::InheritanceMode)?
        .ok_or_else(|| HolonError::EmptyField("InheritanceMode".into()))?;

    match value {
        BaseValue::StringValue(MapString(value))
        | BaseValue::EnumValue(base_types::MapEnumValue(MapString(value))) => {
            match value.as_str() {
                "None" => Ok(InheritanceMode::None),
                "Additive" => Ok(InheritanceMode::Additive),
                "Override" => Ok(InheritanceMode::Override),
                _ => Err(HolonError::InvalidParameter(format!(
                    "Unsupported InheritanceMode '{value}' for InstanceProperties"
                ))),
            }
        }
        other => Err(HolonError::UnexpectedValueType(
            format!("{other:?}"),
            "InheritanceMode enum".into(),
        )),
    }
}

fn local_instance_properties(
    descriptor: &crate::reference_layer::HolonReference,
) -> Result<Vec<PropertyDescriptor>, HolonError> {
    let collection = descriptor.related_holons(CoreRelationshipTypeName::InstanceProperties)?;
    let guard =
        collection.read().map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?;
    let members = guard.get_members();
    Ok(members.iter().cloned().map(PropertyDescriptor::from_holon).collect())
}

fn additive_instance_properties(
    descriptor: &crate::reference_layer::HolonReference,
) -> Result<Vec<PropertyDescriptor>, HolonError> {
    let mut lineage = walk_extends_chain(descriptor).collect::<Result<Vec<_>, _>>()?;
    lineage.reverse();
    let mut members = Vec::new();
    for ancestor in lineage {
        members.extend(local_instance_properties(&ancestor)?);
    }
    Ok(members)
}

fn override_instance_properties(
    descriptor: &crate::reference_layer::HolonReference,
) -> Result<Vec<PropertyDescriptor>, HolonError> {
    for ancestor in walk_extends_chain(descriptor) {
        let local = local_instance_properties(&ancestor?)?;
        if !local.is_empty() {
            return Ok(local);
        }
    }
    Ok(Vec::new())
}

fn normalize_property_members(
    owner: &HolonDescriptor,
    members: Vec<PropertyDescriptor>,
) -> Result<Vec<PropertyDescriptor>, HolonError> {
    let mut by_name = HashMap::new();
    let mut normalized = Vec::new();

    for member in members {
        let name = member.header().type_name()?.to_string();
        let identity = member.holon().reference_id_string();
        if let Some(existing) = by_name.insert(name.clone(), identity.clone()) {
            if existing != identity {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "property".into(),
                    name,
                    descriptor: accessor_helpers::descriptor_label(owner.holon()),
                });
            }
            continue;
        }
        normalized.push(member);
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon, new_test_holon};
    use crate::reference_layer::{ReadableHolon, WritableHolon};
    use type_names::CoreRelationshipTypeName;

    #[test]
    fn available_properties_resolves_the_instance_contract_selected_by_described_by(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let property = new_descriptor_holon(&context, "enabled-property", "Enabled", "Property")?;
        let mut instance_properties_relationship = new_descriptor_holon(
            &context,
            "instance-properties-relationship",
            "InstanceProperties",
            "Relationship",
        )?;
        instance_properties_relationship
            .with_property_value(CorePropertyTypeName::InheritanceMode, "Additive")?;

        let mut meta_descriptor =
            new_descriptor_holon(&context, "meta-type-descriptor", "MetaTypeDescriptor", "Type")?;
        meta_descriptor.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![instance_properties_relationship.into()],
        )?;

        let mut holon_descriptor = new_descriptor_holon(&context, "book-type", "Book", "Type")?;
        holon_descriptor.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![meta_descriptor.into()],
        )?;
        holon_descriptor.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![property.into()],
        )?;

        let mut holon = new_test_holon(&context, "book-instance")?;
        holon.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![holon_descriptor.into()],
        )?;

        let properties = holon.available_properties()?;
        assert_eq!(properties.len(), 1);
        assert_eq!(properties[0].header().type_name()?, MapString("Enabled".into()));
        Ok(())
    }
}
