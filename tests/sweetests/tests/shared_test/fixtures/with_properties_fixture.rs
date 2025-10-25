// tests/sweetests/tests/with_properties_fixture.rs

use crate::shared_test::{test_context::init_fixture_context, test_data_types::DancesTestCase};
use holons_core::reference_layer::holon_operations_api;
use holons_prelude::prelude::*;
use rstest::*;

/// Fixture: stage a holon and then update it via the `with_properties` dance
#[fixture]
pub fn with_properties_fixture() -> Result<DancesTestCase, HolonError> {
    // == Init test case ==
    let mut test_case = DancesTestCase::new(
        "WithProperties Fixture".to_string(),
        "Stages one holon, then updates it via with_properties dance".to_string(),
    );

    // We’ll use a private fixture context to stage data
    let fixture_context = init_fixture_context();

    // Ensure DB starts with only the Space holon
    test_case.add_ensure_database_count_step(MapInteger(1))?;

    // Create a transient holon directly via the transient service (no DanceCallService needed here)
    let transient_service = fixture_context.get_space_manager().get_transient_behavior_service();
    let transient_ref = {
        let borrowed = transient_service.borrow();
        // give it a stable key
        borrowed.create_empty(MapString("WithProps.1".into()))?
    };

    // Stage it (so the dance can target a mutable staged holon)
    let staged_ref =
        holon_operations_api::stage_new_holon(&*fixture_context, transient_ref.clone())?;

    // Build the properties we want to set
    let mut props = PropertyMap::new();
    props.insert(
        PropertyName(MapString("title".into())),
        BaseValue::StringValue(MapString("Updated via dance".into())),
    );
    props.insert(
        PropertyName(MapString("note".into())),
        BaseValue::StringValue(MapString("Hello world".into())),
    );

    // Add the step
    test_case.add_with_properties_step(
        HolonReference::Staged(staged_ref),
        props,
        ResponseStatusCode::OK,
    )?;

    // Optional: verify DB still only has Space + staged in nursery before any commit
    test_case.add_ensure_database_count_step(MapInteger(1))?;

    // Export transient pool into test case session state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case)
}
