use holons_loader_client::load_holons_from_files;
use holons_prelude::prelude::*;
use std::path::PathBuf;

use crate::shared_test::test_data_types::DanceTestExecutionState;

fn read_int_property(
    context: &dyn HolonsContextBehavior,
    reference: &TransientReference,
    property: CorePropertyTypeName,
) -> i64 {
    match reference.property_value(context, &property) {
        Ok(Some(PropertyValue::IntegerValue(MapInteger(i)))) => i,
        Ok(Some(other)) => panic!("Expected integer for {:?}, got {:?}", property, other),
        Ok(None) => panic!("Property {:?} missing on response holon", property),
        Err(err) => panic!("Failed to read {:?} from response holon: {:?}", property, err),
    }
}

/// Execute the loader client end-to-end: validate/parse files, run the dance,
/// and assert loader response properties.
pub async fn execute_load_holons_client(
    test_state: &mut DanceTestExecutionState,
    import_files: Vec<PathBuf>,
    expect_staged: MapInteger,
    expect_committed: MapInteger,
    expect_links_created: MapInteger,
    expect_errors: MapInteger,
    expect_total_bundles: MapInteger,
    expect_total_loader_holons: MapInteger,
) {
    let context = test_state.context();

    let response_reference = load_holons_from_files(context.clone(), &import_files)
        .await
        .unwrap_or_else(|e| panic!("loader_client failed: {e:?}"));

    let ctx = context.as_ref();
    let staged = read_int_property(ctx, &response_reference, CorePropertyTypeName::HolonsStaged);
    let committed =
        read_int_property(ctx, &response_reference, CorePropertyTypeName::HolonsCommitted);
    let links_created =
        read_int_property(ctx, &response_reference, CorePropertyTypeName::LinksCreated);
    let errors = read_int_property(ctx, &response_reference, CorePropertyTypeName::ErrorCount);
    let total_bundles =
        read_int_property(ctx, &response_reference, CorePropertyTypeName::TotalBundles);
    let total_loader_holons =
        read_int_property(ctx, &response_reference, CorePropertyTypeName::TotalLoaderHolons);

    assert_eq!(staged, expect_staged.0);
    assert_eq!(committed, expect_committed.0);
    assert_eq!(links_created, expect_links_created.0);
    assert_eq!(errors, expect_errors.0);
    assert_eq!(total_bundles, expect_total_bundles.0);
    assert_eq!(total_loader_holons, expect_total_loader_holons.0);
}
