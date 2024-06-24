use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use crate::descriptor_types::CoreSchemaName;

pub fn get_core_type_ref (context: &HolonsContext, core_type: CoreSchemaName)
    ->Result<HolonReference, HolonError> {
    let result;
    if let Some(holon_ref) = context.get_by_key_from_dance_state(&core_type.as_map_string())? {
        result = holon_ref;
    } else {
        return Err(HolonError::HolonNotFound(format!("Couldn't find a definition for {} in dance_state", core_type.as_str())));
    }
    Ok(result)
}