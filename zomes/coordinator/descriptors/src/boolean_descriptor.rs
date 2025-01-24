use crate::descriptor_types::{CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName};
use hdi::prelude::debug;

use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};
use holons::reference_layer::{
    HolonReference, HolonWritable, HolonsContextBehavior, StagedReference,
};

use holons::core_shared_objects::stage_new_holon_api;
use holons::core_shared_objects::{Holon, HolonError};
use shared_types_holon::value_types::{BaseType, ValueType};
use shared_types_holon::{BaseValue, MapString, PropertyName};
use CoreSchemaPropertyTypeName::TypeName;

pub struct BooleanTypeDefinition {
    pub header: TypeDescriptorDefinition,
    pub type_name: MapString,
}

/// This function defines (and describes) a new boolean type. Values of this type will be stored
/// as MapBoolean. It has no type-specific properties or relationships. Agent-defined types can be the
/// `ValueType` for a MapProperty.
pub fn define_boolean_type(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    definition: BooleanTypeDefinition,
) -> Result<StagedReference, HolonError> {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let type_descriptor_ref = define_type_descriptor(
        context,
        schema, // should this be type safe (i.e., pass in either Schema or SchemaTarget)?
        BaseType::Value(ValueType::Boolean),
        definition.header.clone(),
    )?;

    // Build the new type

    let mut boolean_type = Holon::new();

    // Add its properties

    boolean_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(definition.type_name.clone()),
        )?
        .with_property_value(
            TypeName.as_property_name(),
            BaseValue::StringValue(definition.type_name),
        )?;

    // Stage the type

    debug!("Staging... {:#?}", boolean_type.clone());

    let boolean_type_ref = stage_new_holon_api(context, boolean_type.clone())?;

    // let boolean_type_ref = {
    //     let staging_behavior = context.get_space_manager().get_staging_behavior_access();
    //     let mut borrowed_staging_behavior = staging_behavior.borrow_mut(); // Borrow mutably
    //     let staged_reference =
    //         borrowed_staging_behavior.stage_new_holon(context, boolean_type.clone())?; // Use it
    //     staged_reference // Return the result to ensure borrow ends here
    // };

    // Add its relationships

    boolean_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor.as_rel_name(),
        vec![HolonReference::Staged(type_descriptor_ref)],
    )?;

    Ok(boolean_type_ref)
}
