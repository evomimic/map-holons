use crate::type_descriptor::{define_type_descriptor, derive_descriptor_name};
use holons::holon_types::Holon;
use holons::relationship::RelationshipTarget;
use shared_types_holon::value_types::BaseType::Holon as BaseTypeHolon;
use shared_types_holon::value_types::{MapBoolean, MapString};

pub fn define_holon_descriptor(
    schema: &RelationshipTarget,
    type_name: MapString,
    description: MapString,
    label: MapString, // Human readable name for this type
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

    // instances: RelationshipTarget,
    // property_descriptors: RelationshipTarget,
    // supertype: RelationshipTarget,
    // source_for: RelationshipTarget,
    // target_for: RelationshipTarget,
    // contained_in: RelationshipTarget
    // dances: RelationshipTarget,
    // constraints: RelationshipTarget,

    descriptor
}
