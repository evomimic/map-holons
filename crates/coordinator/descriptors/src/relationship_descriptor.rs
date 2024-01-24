use holons::holon_types::Holon;
use holons::relationship::RelationshipTarget;
use shared_types_holon::value_types::BaseType::Holon as BaseTypeHolon;
use shared_types_holon::value_types::{MapBoolean, MapInteger, MapString};
use crate::descriptor_types::DeletionSemantic;


use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};

pub fn define_relationship_descriptor(
    schema: &RelationshipTarget,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
    _min_target_cardinality: MapInteger,
    _max_target_cardinality: MapInteger,
    _deletion_semantic: DeletionSemantic,
    _affinity: MapInteger,

) -> Holon {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let descriptor = define_type_descriptor(
        schema,
        derive_descriptor_name(&type_name),
        type_name,
        BaseTypeHolon,
        description,
        label,
        MapBoolean(false),
        MapBoolean(false),
    );

    // Define its PropertyDescriptors
    

    // Properties:
    // relationship_name: StringValue,
    // min_target_cardinality: IntegerValue,
    // max_target_cardinality: IntegerValue,
    // deletion_semantic: DeletionSemantic,
    // affinity: IntegerValue,
    // Relationships
    // supertype: RelationshipTarget,
    // constraints: RelationshipTarget
    // source_holon_type: RelationshipTarget
    // target_holon_type: RelationshipTarget



    descriptor

}