use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::MapString;

use crate::core_schema_types::{CoreValueTypeName, SchemaNamesTrait};

// This file defines and stages (but does not commit) type definitions for all the MAP Core
// ValueTypes.
// pub fn load_core_value_type(
//     context: &HolonsContext,
//     schema: &HolonReference,
//     value_type: CoreValueTypeName,
// ) -> Result<StagedReference, HolonError> {
//     match value_type {
//         CoreValueTypeName::StringType(core_string_value) => {
//             core_string_value.load_core_type(context, schema)
//         }
//         CoreValueTypeName::IntegerType(core_integer_value) => {
//             core_integer_value.load_core_type(context, schema)
//         }
//         CoreValueTypeName::BooleanType(core_boolean_value) => {
//             core_boolean_value.load_core_type(context, schema)
//         }
//         CoreValueTypeName::EnumType(core_enum_value) => {
//             core_enum_value.load_core_type(context, schema)
//         }
//     }
// }
impl SchemaNamesTrait for CoreValueTypeName {
      fn load_core_type(
        &self,
        context: &HolonsContext,
        schema: &HolonReference,
    ) -> Result<StagedReference, HolonError> {
        match self {
            CoreValueTypeName::StringType(inner) => inner.load_core_type(context, schema),
            CoreValueTypeName::IntegerType(inner) => inner.load_core_type(context, schema),
            CoreValueTypeName::BooleanType(inner) => inner.load_core_type(context, schema),
            CoreValueTypeName::EnumType(inner) => inner.load_core_type(context, schema),
        }
    }

    fn derive_type_name(&self) -> MapString {
        match self {
            CoreValueTypeName::StringType(inner) => inner.derive_type_name(),
            CoreValueTypeName::IntegerType(inner) => inner.derive_type_name(),
            CoreValueTypeName::BooleanType(inner) => inner.derive_type_name(),
            CoreValueTypeName::EnumType(inner) => inner.derive_type_name(),
        }
    }

    fn derive_descriptor_name(&self) -> MapString {
        match self {
            CoreValueTypeName::StringType(inner) => inner.derive_descriptor_name(),
            CoreValueTypeName::IntegerType(inner) => inner.derive_descriptor_name(),
            CoreValueTypeName::BooleanType(inner) => inner.derive_descriptor_name(),
            CoreValueTypeName::EnumType(inner) => inner.derive_descriptor_name(),
        }
    }

    fn derive_label(&self) -> MapString {
        match self {
            CoreValueTypeName::StringType(inner) => inner.derive_label(),
            CoreValueTypeName::IntegerType(inner) => inner.derive_label(),
            CoreValueTypeName::BooleanType(inner) => inner.derive_label(),
            CoreValueTypeName::EnumType(inner) => inner.derive_label(),
        }
    }

    fn derive_description(&self) -> MapString {
        match self {
            CoreValueTypeName::StringType(inner) => inner.derive_description(),
            CoreValueTypeName::IntegerType(inner) => inner.derive_description(),
            CoreValueTypeName::BooleanType(inner) => inner.derive_description(),
            CoreValueTypeName::EnumType(inner) => inner.derive_description(),
        }
    }
}