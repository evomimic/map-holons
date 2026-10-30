use crate::descriptors::walk_extends_chain;
use crate::{Descriptor, HolonReference, ReadableHolon};
use core_types::HolonError;

/// Resolve type-owned Slots, allowing a nearer type to override one role.
/// Missing and duplicate role declarations fail rather than choosing arbitrarily.
pub fn composition_slots(visualizer: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    let mut roles = Vec::new();
    let mut result = Vec::new();
    for descriptor in walk_extends_chain(visualizer.holon_descriptor()?.holon()) {
        let descriptor = descriptor?;
        let mut local = Vec::new();
        for slot in super::usage::related(&descriptor, "HasSlot")? {
            let role = match slot.property_value("CompositionRole")? {
                Some(base_types::BaseValue::StringValue(role)) if !role.0.is_empty() => role.0,
                _ => {
                    return Err(HolonError::InvalidParameter(
                        "Missing or malformed CompositionRole".into(),
                    ))
                }
            };
            if local.contains(&role) {
                return Err(HolonError::InvalidState(format!("Duplicate composition role {role}")));
            }
            local.push(role.clone());
            if !roles.contains(&role) {
                roles.push(role);
                result.push(slot);
            }
        }
    }
    Ok(result)
}
