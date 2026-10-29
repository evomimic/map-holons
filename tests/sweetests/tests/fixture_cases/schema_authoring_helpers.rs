//! Stages type-level holons (Schemas, descriptors and validation rules) that a fixture
//! authors itself; `described_instance_helpers` stages instances of saved descriptors.
//! Keys must be unique per fixture because suite fixtures share one bootstrapped runtime.

use super::described_instance_helpers::saved;
use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, FixtureHolons, TestReference};
use std::sync::Arc;

/// Authors complete private definitions; saved schema references remain opaque lookup tokens.
pub(super) fn stage_definition(
    context: &Arc<TransactionContext>,
    case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    key: &str,
    properties: PropertyMap,
    relationships: &[(&str, &TestReference)],
) -> Result<TestReference, HolonError> {
    let key = MapString(key.into());
    let source = context.mutation().new_holon(Some(key.clone()))?;
    let token = case.add_new_holon_step(holons, source, properties, Some(key), None, None)?;
    let mut token = case.add_stage_holon_step(holons, token, None, None)?;
    for (name, target) in relationships {
        token = case.add_add_related_holons_step(
            holons,
            token,
            name.to_relationship_name(),
            vec![(*target).clone()],
            None,
            None,
        )?;
    }
    Ok(token)
}

/// Common explicit descriptor metadata, including the kind and abstractness flags.
pub(super) fn descriptor_properties(name: &str) -> PropertyMap {
    [
        ("TypeName".to_property_name(), name.to_base_value()),
        ("TypeNamePlural".to_property_name(), format!("{name}s").to_base_value()),
        ("DisplayName".to_property_name(), name.to_base_value()),
        ("DisplayNamePlural".to_property_name(), format!("{name}s").to_base_value()),
        ("Description".to_property_name(), "Private descriptor fixture".to_base_value()),
        ("DefinesInstanceTypeKind".to_property_name(), false.to_base_value()),
        ("IsAbstractType".to_property_name(), false.to_base_value()),
    ]
    .into()
}

/// Stages a private required string property with an exact Schema owner.
pub(super) fn stage_title(
    context: &Arc<TransactionContext>,
    case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    key: &str,
    owner: &TestReference,
) -> Result<TestReference, HolonError> {
    let root = saved(context, case, holons, "PropertyType.TypeDescriptor")?;
    let meta = saved(context, case, holons, "MetaPropertyType.MetaTypeDescriptor")?;
    let value = saved(context, case, holons, "MapStringValueType.StringValueType")?;
    let mut properties = descriptor_properties("Title");
    properties.insert("IsValueRequired".to_property_name(), true.to_base_value());
    stage_definition(
        context,
        case,
        holons,
        key,
        properties,
        &[
            ("Extends", &root),
            ("ComponentOf", owner),
            ("ValueType", &value),
            ("DescribedBy", &meta),
        ],
    )
}

/// Replaces one authored forward target through the ordinary relationship steps.
pub(super) fn replace_target(
    case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    source: TestReference,
    relationship: &str,
    old: &TestReference,
    new: &TestReference,
) -> Result<TestReference, HolonError> {
    let source = case.add_remove_related_holons_step(
        holons,
        source,
        relationship.to_relationship_name(),
        vec![old.clone()],
        None,
        None,
    )?;
    case.add_add_related_holons_step(
        holons,
        source,
        relationship.to_relationship_name(),
        vec![new.clone()],
        None,
        None,
    )
}
