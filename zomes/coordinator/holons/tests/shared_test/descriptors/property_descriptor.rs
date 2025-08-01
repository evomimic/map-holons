use holons::helpers::define_local_target;
use holons::holon_reference::HolonReference::*;
use holons::holon_reference::{HolonReference, LocalHolonReference};
use holons::holon_types::Holon;
use holons::relationship::HolonCollection;
use holons::relationship::HolonCollection::*;
use integrity_core_types::holon_node::PropertyName;
use integrity_core_types::value_types::{
    TypeKind, BaseValue, MapBoolean, MapEnumValue, MapInteger, MapString,
};

/// This file creates a HolonDescriptor Holon and its Associated Relationships
/// We can greatly reduce the code-bulk if we re-factored this as an import function that takes
/// a JSON input stream with type definitions expressed as JSON objects.

// This function defines the TypeDescriptor for a HolonDescriptor (but not the HolonDescriptor itself
pub fn define_property_type_descriptor() -> Holon {
    // ----------------  DEFINE THE
    // META HOLON DESCRIPTOR -------------------------------
    let mut type_descriptor = Holon::new();

    type_descriptor
        .with_property_value(
            PropertyName(MapString("type_name".to_string())),
            BaseValue::StringValue(MapString("HolonDescriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Describes the characteristics of Holons".to_string(),
            )),
        )
        .with_property_value(
            PropertyName(MapString("label".to_string())),
            BaseValue::StringValue(MapString("Holon Descriptor".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("base_type".to_string())),
            BaseValue::StringValue(MapString("TypeKind::Holon".to_string())),
        )
        .with_property_value(
            PropertyName(MapString("is_dependent".to_string())),
            BaseValue::BooleanValue(MapBoolean(false)),
        )
        .with_property_value(
            PropertyName(MapString("is_built_in".to_string())),
            BaseValue::BooleanValue(MapBoolean(true)),
        );

    type_descriptor
}

