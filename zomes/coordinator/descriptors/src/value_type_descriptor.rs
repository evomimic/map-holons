use crate::descriptor_types::CoreValueTypeName;
use holons::core_shared_objects::HolonError;
use holons::reference_layer::{HolonReference, HolonsContextBehavior};

use shared_types_holon::MapString;

// TODO: Enhance to do a get from cache if not in dance_state
pub fn get_core_value_type_descriptor_reference(
    context: &dyn HolonsContextBehavior,
    value_type: CoreValueTypeName,
) -> Result<HolonReference, HolonError> {
    let key = MapString(value_type.as_str().to_string());
    context.get_by_key_from_transient_state(&key)?.ok_or_else(|| {
        HolonError::HolonNotFound(format!(
            "Couldn't find StagedReference for {:?} in dance_state",
            value_type.as_str()
        ))
    })
}

/*
pub fn define_value_type(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    descriptor_name: MapString,
    type_name: MapString,
    value_type: ValueType,
    description: MapString,
    label: MapString, // Human-readable name for this type
    described_by: Option<HolonReference>,
    has_supertype: Option<HolonReference>
    //_owned_by: HolonReference, // HolonSpace
) -> Result<StagedReference, HolonError> {

    // ------------ Validate the supplied parameters
    // Skip type-checking the schema HolonReference, if it does not reference a Schema Holon,
    // validation on the COMPONENT_OF relationship will fail




    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut descriptor = Holon::new();

    let base_type = BaseType::Value(value_type);


    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let mut descriptor = define_type_descriptor(
        context,
        schema,
        descriptor_name,
        type_name,
        base_type,
        description,
        label,
        MapBoolean(true), // is_dependent
        MapBoolean(true), // is_value_type
        described_by,
        has_supertype,
    )?;

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(type_name),
        )
        .with_property_value(
            PropertyName(MapString("descriptor_name".to_string())),
            BaseValue::StringValue(descriptor_name),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(description),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(label),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::EnumValue(MapEnumValue(MapString(base_type.to_string()))),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(MapBoolean(true)),
        )
        .with_property_value(
            PropertyName(MapString("is_value_descriptor".to_string())),
            BaseValue::BooleanValue(MapBoolean(true)),
        );

    // Define a default semantic_version
    let _version = set_semantic_version(0, 0, 1);


    // Add the outbound relationships shared by all ValueTypeDescriptors
   // let version_target = define_local_target(&version);

    // descriptor
    //     .add_related_holon(
    //         RelationshipName(MapString("COMPONENT_OF".to_string())),
    //         schema_target,
    //     )
    //     .add_related_holon(
    //         RelationshipName(MapString("VERSION".to_string())),
    //         version_target,
    //     );

    // TODO: If has_supertype is supplied, populate that relationship

    // if let Some(supertype) = has_supertype  {
    //     let supertype_reference = HolonReference::Local(LocalHolonReference::from_holon(supertype.0.clone()));
    //     descriptor.add_related_holon(
    //         RelationshipName(MapString("HAS_SUPERTYPE".to_string())),
    //         RelationshipTarget::ZeroOrOne(Some(supertype_reference)),
    //     );
    // }
    // // TODO: If described_by is supplied, populate that relationship
    // if let Some(is_described_by) = described_by  {
    //     let described_by_reference = HolonReference::Local(LocalHolonReference::from_holon(is_described_by.0.clone()));
    //
    //     descriptor
    //         .add_related_holon(
    //         RelationshipName(MapString("DESCRIBED_BY".to_string())),
    //         RelationshipTarget::ZeroOrOne(Some(described_by_reference)),
    //     );
    // }
    //TODO: Populate owned_by relationship
    // descriptor.add_related_holon(
    //     RelationshipName(MapString("OwnedBy".to_string())),
    //     owned_by.clone(),

    TypeDescriptor(descriptor)
}
*/
