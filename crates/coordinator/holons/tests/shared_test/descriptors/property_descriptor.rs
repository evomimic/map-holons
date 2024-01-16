use holons::helpers::define_local_target;
use holons::holon_reference::{HolonReference, LocalHolonReference};
use holons::holon_reference::HolonReference::*;
use holons::holon_types::{Holon};
use holons::relationship::RelationshipTarget;
use holons::relationship::RelationshipTarget::*;
use shared_types_holon::value_types::BaseType;

use shared_types_holon::holon_node::{BaseValue};


/// This file creates a HolonDescriptor Holon and its Associated Relationships
/// We can greatly reduce the code-bulk if we re-factored this as an import function that takes
/// a JSON input stream with type definitions expressed as JSON objects.

// This function defines the TypeDescriptor for a HolonDescriptor (but not the HolonDescriptor itself
pub fn define_property_type_descriptor() -> Holon {

    // ----------------  DEFINE THE
    // META HOLON DESCRIPTOR -------------------------------
    let mut type_descriptor = Holon::new();


    type_descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("HolonDescriptor".to_string()))
        .with_property_value("description".to_string(), BaseValue::StringValue(
            "Describes the characteristics of Holons".to_string()))
        .with_property_value("label".to_string(), BaseValue::StringValue("Holon Descriptor".to_string()))
        .with_property_value("base_type".to_string(), BaseValue::StringValue("BaseType::Holon".to_string()))
        .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(false))
        .with_property_value("is_built_in".to_string(), BaseValue::BooleanValue(true));

    type_descriptor
}
// Defines a HolonDescriptor (w/o any relationships)
pub fn define_property_descriptor() -> Holon {

      let mut descriptor = Holon::new();

    descriptor
}
