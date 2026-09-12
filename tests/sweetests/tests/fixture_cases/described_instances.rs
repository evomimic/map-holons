//! Small schema-conforming setups; each scenario stages only the subjects it needs.

use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, FixtureHolons, TestReference};
use std::sync::Arc;

/// Stages one instance with a declared string property and a saved descriptor.
pub fn add_described_instance(
    context: &Arc<TransactionContext>,
    test_case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    key: &str,
    property: &str,
    descriptor_key: &str,
) -> Result<TestReference, HolonError> {
    let descriptor_key = MapString(descriptor_key.into());
    let stub = context.mutation().new_holon(Some(descriptor_key.clone()))?;
    let descriptor =
        test_case.add_lookup_saved_holon_by_key_step(holons, stub, descriptor_key, None, None)?;
    let key = MapString(key.into());
    let source = context.mutation().new_holon(Some(key.clone()))?;
    let mut properties = PropertyMap::new();
    properties.insert(property.to_property_name(), key.clone().to_base_value());
    let token = test_case.add_new_holon_step(holons, source, properties, Some(key), None, None)?;
    let token = test_case.add_stage_holon_step(holons, token, None, None)?;
    test_case.add_add_related_holons_step(
        holons,
        token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![descriptor],
        None,
        None,
    )
}
