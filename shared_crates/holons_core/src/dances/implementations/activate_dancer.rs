use std::sync::Arc;

use base_types::{BaseValue, MapString};
use core_types::HolonError;
use type_names::{DancerPropertyTypeName, ToPropertyName};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::BoundDanceInvocation;
use crate::reference_layer::ReadableHolon;

/// Adapts the generic ActivateDancer contract to the context-specific package
/// activation strategy. Package resolution and loading remain host concerns.
pub fn invoke(
    context: &Arc<TransactionContext>,
    bound_invocation: &BoundDanceInvocation,
) -> Result<Option<crate::reference_layer::HolonReference>, HolonError> {
    let request =
        bound_invocation.request().ok_or_else(|| HolonError::MissingRequiredRelationship {
            relationship: "Request".to_string(),
            descriptor: "ActivateDancer.DanceInvocation".to_string(),
        })?;
    let property = DancerPropertyTypeName::DancerPackageIdentity.to_property_name();
    let package_identity = match request.property_value(property.clone())? {
        Some(BaseValue::StringValue(value)) => value,
        Some(other) => {
            return Err(HolonError::UnexpectedValueType(
                format!("{other:?}"),
                "String".to_string(),
            ));
        }
        None => return Err(HolonError::EmptyField(property.0 .0)),
    };

    context.activate_dancer(&MapString(package_identity.0))?;
    Ok(None)
}
