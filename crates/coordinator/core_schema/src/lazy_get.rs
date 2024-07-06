use descriptors::descriptor_types::CoreValueTypeName;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use shared_types_holon::MapString;
use crate::core_schema_types::{CoreSchemaTypeName, SchemaNamesTrait};


/// This function is used to get a HolonReference to the TypeDefinition for a CoreSchemaTypeName
/// It first checks if that definition has been stashed in dance_state.
/// If not, it searches the persistent store for TypeDefinition whose key is `desired_type_name`
/// If still not found, it invokes the core_type_loader method on desired_type_name to stage
/// the desired type and return a HolonReference to the staged holon.
pub fn lazy_get_core_descriptor(
  context: &HolonsContext,
  schema:&HolonReference,
  desired_type_name: CoreSchemaTypeName,
) -> Result<HolonReference, HolonError> {

    let key = desired_type_name.derive_type_name();
    let descriptor_reference = context.get_by_key_from_dance_state(&key)?;
    //   .ok_or_else(|| HolonError::HolonNotFound(format!("Couldn't find StagedReference for {:?} in dance_state", value_type.as_str())))

    return Err(HolonError::NotImplemented("lazy_get_type_descriptor_reference is not yet implemented.".to_string()))

}


// pub fn get_core_value_type_descriptor_reference(context: &HolonsContext, value_type: CoreValueTypeName) -> Result<HolonReference, HolonError> {
//   let key = MapString(value_type.as_str().to_string());
//   context.get_by_key_from_dance_state(&key)?
//       .ok_or_else(|| HolonError::HolonNotFound(format!("Couldn't find StagedReference for {:?} in dance_state", value_type.as_str())))
// }
