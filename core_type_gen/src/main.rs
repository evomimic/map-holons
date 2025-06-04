use crate::templates::enum_template::ENUM_TEMPLATE;
use std::fs;
use std::path::Path;
use std::process;

mod generate;
mod parse;
mod templates;

use generate::holon_types::generate_json_specs_from_yaml;
use generate::template::generate_enum_from_template;
use parse::holon_types::parse_holon_types_yaml;

fn main() {
    let yaml_path = Path::new("core_type_gen/core_type_defs/holon_types.yml");
    let enum_output_path =
        Path::new("crates/type_system/type_names/src/generated/core_holon_type_name.rs");
    let json_output_dir = Path::new("core_type_gen/generated_specs/holon_types/");

    println!("📦 Parsing {:?}", yaml_path);
    let holon_file = match parse_holon_types_yaml(yaml_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("❌ Error parsing YAML: {e}");
            process::exit(1);
        }
    };

    let variant_names: Vec<String> =
        holon_file.variants.iter().map(|entry| entry.variant.clone()).collect();

    if let Some(parent) = enum_output_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create enum output directory");
    }

    println!("🛠️  Generating enum to {:?}", enum_output_path);
    if let Err(e) =
        generate_enum_from_template("CoreHolonTypeName", &variant_names, enum_output_path)
    {
        eprintln!("❌ Failed to generate enum: {e}");
        process::exit(1);
    }

    println!("📁 Generating JSON specs to {:?}", json_output_dir);
    if let Err(e) = generate_json_specs_from_yaml(&holon_file, json_output_dir) {
        eprintln!("❌ Failed to generate JSON specs: {e}");
        process::exit(1);
    }

    println!("✅ Enum and JSON spec generation complete.");
}
