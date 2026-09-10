use std::sync::Arc;

use core_types::HolonError;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::BoundDanceInvocation;
use crate::reference_layer::HolonReference;

/// Executes the MaterializeVisualizer Dance through the context's Holon
/// service strategy.
pub fn invoke(
    context: &Arc<TransactionContext>,
    bound_invocation: &BoundDanceInvocation,
) -> Result<Option<HolonReference>, HolonError> {
    let visualizer = bound_invocation.affording_holon().ok_or_else(|| {
        HolonError::MissingRequiredRelationship {
            relationship: "AffordingHolon".to_string(),
            descriptor: "MaterializeVisualizer.DanceInvocation".to_string(),
        }
    })?;

    context.materialize_visualizer(visualizer).map(Some)
}
