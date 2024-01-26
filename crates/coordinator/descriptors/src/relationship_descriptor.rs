use holons::holon_reference::HolonReference;
use holons::holon_types::Holon;
use holons::relationship::{RelationshipName, RelationshipTarget};
use shared_types_holon::PropertyName;
use shared_types_holon::value_types::BaseType::Holon as BaseTypeHolon;
use shared_types_holon::value_types::{BaseValue, MapBoolean, MapEnumValue, MapInteger, MapString};
use crate::descriptor_types::DeletionSemantic;


use crate::type_descriptor::{define_type_descriptor};

/// This function defines and stages (but does not persist) a new RelationshipDescriptor.
/// Values for each of the RelationshipDescriptor properties will be set based on supplied parameters.
///
/// *Naming Rules*:
///     `type_name` := <source_for.type_name>"-"<relationship_name>"->"<target_for.type_name>"
///     `descriptor_name`:= `<type_name>"Descriptor"`
///
/// The descriptor will have the following relationships populated:
/// * DESCRIBED_BY->TypeDescriptor (if supplied)
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * HAS_SUPERTYPE-> HolonDescriptor (if supplied)
/// *
///
///
pub fn define_relationship_descriptor(
    schema: &RelationshipTarget,
    relationship_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    min_target_cardinality: MapInteger,
    max_target_cardinality: MapInteger,
    deletion_semantic: DeletionSemantic,
    affinity: MapInteger,
    source_for: RelationshipTarget, // TODO: switch type to HolonReference
    target_for: RelationshipTarget, // TODO: switch type to HolonReference
    has_supertype: Option<HolonReference>,
    described_by: Option<HolonReference>,

) -> Holon {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let type_name= MapString(format!("{}-{}->{}", "source_for_type_name".to_string(), relationship_name.0,"target_for_type_name".to_string()));
    let mut descriptor = define_type_descriptor(
        schema,
        MapString(format!("{}{}", type_name.0, "Descriptor".to_string())),
        type_name,
        BaseTypeHolon,
        description,
        label,
        MapBoolean(false),
        MapBoolean(false),
        described_by,
        has_supertype,
    );

    // Add its properties

    descriptor
        .with_property_value(
            PropertyName(MapString("min_target_cardinality".to_string())),
            BaseValue::IntegerValue(min_target_cardinality),
        )
        .with_property_value(
            PropertyName(MapString("max_target_cardinality".to_string())),
            BaseValue::IntegerValue(max_target_cardinality),
        )
        .with_property_value(
            PropertyName(MapString("deletion_semantic".to_string())),
            BaseValue::EnumValue(deletion_semantic.to_enum_variant()),
        )
        .with_property_value(
            PropertyName(MapString("affinity".to_string())),
            BaseValue::IntegerValue(affinity),
        );


    // Populate its relationships
    // _source_for: HolonReference,
    //     _target_for: HolonReference,
    //     _has_supertype: Option<HolonReference>,
    descriptor
        .add_related_holon(
            RelationshipName(MapString("COMPONENT_OF".to_string())),
            schema.clone(),
        )
        .add_related_holon(
            RelationshipName(MapString("SOURCE_FOR".to_string())),
            source_for.clone(),
        )
        .add_related_holon(
            RelationshipName(MapString("TARGET_FOR".to_string())),
            target_for.clone(),
        );

    // TODO: If has_supertype is supplied, populate that relationship
    // if let Some(supertype) = has_supertype  {
    //     descriptor.add_related_holon(
    //         RelationshipName(MapString("HAS_SUPERTYPE".to_string())),
    //         supertype.clone(),
    //     )
    // }
    // TODO: If described_by is supplied, populate that relationship
    // if let Some(is_described_by) = described_by  {
    //     descriptor.add_related_holon(
    //         RelationshipName(MapString("DESCRIBED_BY".to_string())),
    //         is_described_by.clone(),
    //     )
    // }




    descriptor

}