use crate::templates::enum_template::ENUM_TEMPLATE;
use handlebars::Handlebars;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
pub struct EnumTemplateContext {
    pub enum_name: String,
    pub first_variant: String,
    pub other_variants: Vec<String>,
    pub all_variants: Vec<String>,
    pub source_file: String,
}

pub fn generate_enum_from_template(
    enum_name: &str,
    variants: &[String],
    out_path: &Path,
) -> Result<(), String> {
    if variants.is_empty() {
        return Err("No variants provided".into());
    }

    let (first, rest) = variants.split_first().unwrap();

    let mut handlebars = Handlebars::new();
    handlebars
        .register_template_string("enum", ENUM_TEMPLATE)
        .map_err(|e| format!("Template registration error: {e}"))?;

    let context = EnumTemplateContext {
        enum_name: format!("{}Name", enum_name),
        first_variant: first.clone(),
        other_variants: rest.to_vec(),
        all_variants: variants.to_vec(),
        source_file: "enum_template.rs".to_string(),
    };

    let rendered =
        handlebars.render("enum", &context).map_err(|e| format!("Template render error: {e}"))?;

    fs::write(out_path, rendered).map_err(|e| format!("Failed to write enum file: {e}"))?;

    Ok(())
}
