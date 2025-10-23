use crate::shared_test::test_data_types::{DanceTestExecutionState, DanceTestStep};

use holons_prelude::prelude::*;

fn read_string_prop(
    ctx: &dyn HolonsContextBehavior,
    r: &TransientReference,
    p: CorePropertyTypeName,
) -> Result<String, HolonError> {
    let v = r.property_value(ctx, &p.as_property_name())?;
    match v {
        Some(PropertyValue::StringValue(s)) => Ok(s.0),
        other => Err(HolonError::InvalidParameter(format!(
            "Expected string for {:?}, got {:?}",
            p, other
        ))),
    }
}

fn read_int_prop(
    ctx: &dyn HolonsContextBehavior,
    r: &TransientReference,
    p: CorePropertyTypeName,
) -> Result<i64, HolonError> {
    let v = r.property_value(ctx, &p.as_property_name())?;
    match v {
        Some(PropertyValue::IntegerValue(MapInteger(i))) => Ok(i),
        other => Err(HolonError::InvalidParameter(format!(
            "Expected integer for {:?}, got {:?}",
            p, other
        ))),
    }
}

/// Execute the LoadHolons step via the Dance façade (guest path).
pub async fn execute_load_holons<C>(
    state: &mut DanceTestExecutionState<C>,
    bundle: TransientReference,
    expect_status: &str,
    expect_staged: usize,
    expect_committed: usize,
    expect_links_created: usize,
    expect_errors: usize,
) -> Result<(), HolonError> {
    let ctx = state.context();

    // Use the façade you threaded into the test state
    let dance = state.dance_call_service.as_ref();

    // Delegate to the client op → dancer → guest controller
    let response_ref = holon_operations_api::load_holons(ctx, bundle, Some(dance))?;

    // --- assertions on response holon props ---
    let status = read_string_prop(ctx, &response_ref, P::ResponseStatusCode)?;
    let staged = read_int_prop(ctx, &response_ref, P::HolonsStaged)? as usize;
    let committed = read_int_prop(ctx, &response_ref, P::HolonsCommitted)? as usize;
    let links_created = read_int_prop(ctx, &response_ref, P::LinksCreated)? as usize;
    let error_count = read_int_prop(ctx, &response_ref, P::ErrorCount)? as usize;

    if status != expect_status {
        return Err(HolonError::InvalidParameter(format!(
            "Expected status {}, got {}",
            expect_status, status
        )));
    }
    if staged != expect_staged {
        return Err(HolonError::InvalidParameter(format!(
            "Expected HolonsStaged={}, got {}",
            expect_staged, staged
        )));
    }
    if committed != expect_committed {
        return Err(HolonError::InvalidParameter(format!(
            "Expected HolonsCommitted={}, got {}",
            expect_committed, committed
        )));
    }
    if links_created != expect_links_created {
        return Err(HolonError::InvalidParameter(format!(
            "Expected LinksCreated={}, got {}",
            expect_links_created, links_created
        )));
    }
    if error_count != expect_errors {
        return Err(HolonError::InvalidParameter(format!(
            "Expected ErrorCount={}, got {}",
            expect_errors, error_count
        )));
    }

    Ok(())
}

// (Tiny router so dance_tests.rs can call us.)
pub async fn route_step<C>(
    state: &mut DanceTestExecutionState<C>,
    step: &DanceTestStep,
) -> Result<(), HolonError> {
    match step {
        DanceTestStep::LoadHolons {
            bundle,
            expect_status,
            expect_staged,
            expect_committed,
            expect_links_created,
            expect_errors,
        } => {
            execute_load_holons(
                state,
                bundle.clone(),
                expect_status,
                *expect_staged,
                *expect_committed,
                *expect_links_created,
                *expect_errors,
            )
            .await
        }
        _ => Ok(()),
    }
}
