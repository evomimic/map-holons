use std::collections::HashMap;

use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, HolonId, LocalId, PropertyMap};
use type_names::{
    CorePropertyTypeName, CoreRelationshipTypeName, ToPropertyName, ToRelationshipName,
};

use crate::core_shared_objects::holon::SavedHolon;
use crate::descriptors::test_support::{
    build_context, build_context_with_saved_holons, new_test_holon,
};
use crate::{HolonReference, ReadableHolon, WritableHolon};

fn id(value: u8) -> HolonId {
    HolonId::Local(LocalId(vec![value; 39]))
}

fn properties(values: &[(&str, &str)]) -> PropertyMap {
    values
        .iter()
        .map(|(name, value)| {
            (name.to_property_name(), BaseValue::StringValue(MapString((*value).to_owned())))
        })
        .collect()
}

#[test]
fn undescribed_properties_use_the_same_effective_contract_in_every_phase() -> Result<(), HolonError>
{
    // The instance type inherits Key and contributes Title locally. Optional is
    // described but absent; it must not appear among undescribed populated names.
    let snapshots = [
        (
            1,
            properties(&[
                ("Key", "instance"),
                ("Title", "Example"),
                ("ZExtra", "z"),
                ("AExtra", "a"),
            ]),
        ),
        (2, properties(&[("Key", "child-type"), ("TypeName", "Child")])),
        (3, properties(&[("Key", "parent-type"), ("TypeName", "Parent")])),
        (4, properties(&[("Key", "key-property"), ("TypeName", "Key")])),
        (5, properties(&[("Key", "title-property"), ("TypeName", "Title")])),
        (6, properties(&[("Key", "optional-property"), ("TypeName", "Optional")])),
        (7, properties(&[("Title", "Keyless example")])),
    ]
    .into_iter()
    .map(|(value, properties)| {
        SavedHolon::new(LocalId(vec![value; 39]), properties, None, MapInteger(1))
    })
    .collect();
    let relationships = HashMap::from([
        ((id(1), CoreRelationshipTypeName::DescribedBy.to_relationship_name()), vec![id(2)]),
        ((id(7), CoreRelationshipTypeName::DescribedBy.to_relationship_name()), vec![id(2)]),
        ((id(2), CoreRelationshipTypeName::Extends.to_relationship_name()), vec![id(3)]),
        (
            (id(2), CoreRelationshipTypeName::InstanceProperties.to_relationship_name()),
            vec![id(5), id(6)],
        ),
        ((id(3), CoreRelationshipTypeName::InstanceProperties.to_relationship_name()), vec![id(4)]),
    ]);
    let context = build_context_with_saved_holons(snapshots, relationships);
    let smart = HolonReference::smart_from_id(context.context_handle(), id(1));
    let descriptor = HolonReference::smart_from_id(context.context_handle(), id(2));
    let mut transient = new_test_holon(&context, "transient-instance")?;
    transient
        .with_property_value("Title", "Example")?
        .with_property_value("ZExtra", "z")?
        .with_property_value("AExtra", "a")?;
    transient.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![descriptor])?;
    let expected = vec!["AExtra".to_property_name(), "ZExtra".to_property_name()];
    assert_eq!(transient.undescribed_property_names()?, expected);
    let staged = context.mutation().stage_new_holon(transient.clone())?;
    assert_eq!(staged.undescribed_property_names()?, expected);
    assert_eq!(smart.undescribed_property_names()?, expected);
    assert_eq!(HolonReference::from(transient).undescribed_property_names()?, expected);
    assert_eq!(HolonReference::from(staged).undescribed_property_names()?, expected);
    let keyless = HolonReference::smart_from_id(context.context_handle(), id(7));
    assert_eq!(keyless.key()?, None);
    assert!(keyless.undescribed_property_names()?.is_empty());
    Ok(())
}

#[test]
fn undescribed_properties_propagate_missing_and_multiple_descriptors() -> Result<(), HolonError> {
    let context = build_context();
    let mut holon = new_test_holon(&context, "instance")?;
    assert!(matches!(
        holon.undescribed_property_names(),
        Err(HolonError::MissingDescribedBy { .. })
    ));
    let first = new_test_holon(&context, "first")?;
    let second = new_test_holon(&context, "second")?;
    holon.add_related_holons(
        CoreRelationshipTypeName::DescribedBy,
        vec![first.into(), second.into()],
    )?;
    assert!(matches!(
        holon.undescribed_property_names(),
        Err(HolonError::MultipleDescribedBy { count: 2, .. })
    ));
    // Reading conformance must not mutate populated state.
    assert_eq!(
        holon.property_value(CorePropertyTypeName::Key)?,
        Some(BaseValue::StringValue(MapString("instance".into())))
    );
    Ok(())
}
