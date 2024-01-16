use holons::holon_types::Holon;
use holons::relationship::RelationshipTarget;
use shared_types_holon::value_types::BaseType::Holon as BaseTypeHolon;
use crate::type_descriptor::define_type_descriptor;

pub fn define_holon_descriptor(
    schema: &RelationshipTarget,
    type_name: String,
    description: String,
    label: String, // Human readable name for this type

) -> Holon {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let descriptor = define_type_descriptor(
        schema,
        type_name,
        BaseTypeHolon,
        description,
        label,
        false,
        false,
    );



    /// instances: RelationshipTarget,
    // property_descriptors: RelationshipTarget,
    // supertype: RelationshipTarget,
    // source_for: RelationshipTarget,
    // target_for: RelationshipTarget,
    // contained_in: RelationshipTarget
    // dances: RelationshipTarget,
    // constraints: RelationshipTarget,



    descriptor

}