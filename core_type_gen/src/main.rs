mod generate;
mod parse;
mod templates;

use crate::generate::core_type_generator::generate_enum_and_json;
use crate::parse::boolean_types::BooleanTypesFile;
use crate::parse::holon_types::HolonTypesFile;
use crate::parse::integer_types::IntegerTypesFile;
use crate::parse::property_types::PropertyTypesFile;
use crate::parse::relationship_types::RelationshipTypesFile;
use crate::parse::string_types::StringTypesFile;
use crate::parse::type_kind_parser::ParseTypeKind;
use serde::Serialize;
use std::path::Path;
// + other type kinds...

fn main() -> Result<(), String> {
    generate_enum_and_json::<HolonTypesFile>(
        Path::new("core_type_gen/core_type_defs/holon_types.yml"),
        Path::new("crates/type_system/type_names/src/generated/core_holon_type_name.rs"),
        Path::new("core_type_gen/generated_specs/holon_types/"),
        "CoreHolonType",
        |file| file.variants.iter().map(|v| v.variant.clone()).collect(),
    )?;

    generate_enum_and_json::<PropertyTypesFile>(
        Path::new("core_type_gen/core_type_defs/property_types.yml"),
        Path::new("crates/type_system/type_names/src/generated/core_property_type_name.rs"),
        Path::new("core_type_gen/generated_specs/property_types/"),
        "CorePropertyType",
        |file| file.variants.iter().map(|v| v.variant.clone()).collect(),
    )?;

    generate_enum_and_json::<StringTypesFile>(
        Path::new("core_type_gen/core_type_defs/string_types.yml"),
        Path::new("crates/type_system/type_names/src/generated/core_string_type_name.rs"),
        Path::new("core_type_gen/generated_specs/string_types/"),
        "CoreStringType",
        |file| file.variants.iter().map(|v| v.variant.clone()).collect(),
    )?;

    generate_enum_and_json::<RelationshipTypesFile>(
        Path::new("core_type_gen/core_type_defs/relationship_types.yml"),
        Path::new("crates/type_system/type_names/src/generated/core_relationship_type_name.rs"),
        Path::new("core_type_gen/generated_specs/relationship_types/"),
        "CoreRelationshipType",
        |file| file.variants.iter().map(|v| v.variant.clone()).collect(),
    )?;

    generate_enum_and_json::<IntegerTypesFile>(
        Path::new("core_type_gen/core_type_defs/integer_types.yml"),
        Path::new("crates/type_system/type_names/src/generated/core_integer_type_name.rs"),
        Path::new("core_type_gen/generated_specs/integer_types/"),
        "CoreIntegerType",
        |file| file.variants.iter().map(|v| v.variant.clone()).collect(),
    )?;

    generate_enum_and_json::<BooleanTypesFile>(
        Path::new("core_type_gen/core_type_defs/boolean_types.yml"),
        Path::new("crates/type_system/type_names/src/generated/core_boolean_type_name.rs"),
        Path::new("core_type_gen/generated_specs/boolean_types/"),
        "CoreBooleanType",
        |file| file.variants.iter().map(|v| v.variant.clone()).collect(),
    )?;

    Ok(())
}
