use holons::holon_types::Holon;
use holons::relationship::RelationshipTarget;
use shared_types_holon::value_types::BaseType::Holon as BaseTypeHolon;
use shared_types_holon::value_types::MapInteger;
use crate::descriptor_types::DeletionSemantic;


use crate::type_descriptor::define_type_descriptor;

pub fn define_relationship_descriptor(
    schema: &RelationshipTarget,
    type_name: String,
    description: String,
    label: String, // Human readable name for this type
    min_target_cardinality: MapInteger,
    max_target_cardinality: MapInteger,
    deletion_semantic: DeletionSemantic,
    affinity: MapInteger,

) -> Holon {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let mut descriptor = define_type_descriptor(
        schema,
        type_name,
        BaseTypeHolon,
        description,
        label,
        false,
        false,
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