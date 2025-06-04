use crate::parse::holon_types::HolonTypesFile;
use handlebars::Handlebars;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct EnumTemplateContext {
    enum_name: String,
    variants: Vec<String>,
    source: String,
}

pub fn generate_holon_types_enum(
    input: &HolonTypesFile,
    template: &Handlebars,
    out_path: &Path,
) -> Result<(), String> {
    let context = EnumTemplateContext {
        enum_name: "CoreHolonTypeName".into(),
        variants: input.variants.iter().map(|entry| entry.variant.clone()).collect(),
        source: "holon_types.yml".into(),
    };

    let rendered = template
        .render("enum", &context)
        .map_err(|e| format!("Failed to render enum template: {e}"))?;

    fs::write(out_path, rendered)
        .map_err(|e| format!("Failed to write enum file to {:?}: {e}", out_path))?;

    println!("✅ Generated CoreHolonTypeName enum at {:?}", out_path);
    Ok(())
}

pub fn generate_json_specs_from_yaml(
    holon_file: &HolonTypesFile,
    output_dir: &Path,
) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output dir {:?}: {}", output_dir, e))?;

    for entry in &holon_file.variants {
        let file_path = output_dir.join(format!("{}.json", entry.variant));
        let json = serde_json::to_string_pretty(&entry)
            .map_err(|e| format!("Failed to serialize {}: {e}", entry.variant))?;
        fs::write(&file_path, json)
            .map_err(|e| format!("Failed to write to {:?}: {e}", file_path))?;
    }

    println!("✅ JSON spec files written to {:?}", output_dir);
    Ok(())
}
