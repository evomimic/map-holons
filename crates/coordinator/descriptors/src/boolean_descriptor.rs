use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::value_types::{BaseType, ValueType};

use crate::type_descriptor::{define_type_descriptor, TypeDefinitionHeader};

/// This function defines (and describes) a new boolean type. Values of this type will be stored
/// as MapBoolean. It has no type-specific properties or relationships. Agent-defined types can be the
/// `ValueType` for a MapProperty.
pub fn define_boolean_type(
    context: &HolonsContext,
    schema: &HolonReference,
    header: TypeDefinitionHeader,
) -> Result<StagedReference, HolonError> {

    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let descriptor = define_type_descriptor(
        context,
        schema, // should this be type safe (i.e., pass in either Schema or SchemaTarget)?
        BaseType::Value(ValueType::Boolean),
        header,
    )?;

    Ok(descriptor)

}