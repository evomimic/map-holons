use crate::{
    literal_bridge::{literal_object_to_json_map, literal_value_to_json},
    loader_ir::{LoaderDocument, LoaderHolon, LoaderMeta, LoaderReference, LoaderRelationship},
    schema_ir::{DescriptorKind, Schema, SemanticModel, SemanticReference, TypeDescriptor},
};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

const INDENT: &str = "  ";

#[derive(Debug, Default)]
pub struct EmittedKeyLookup {
    emitted_keys: HashSet<String>,
    unique_keys_by_name: HashMap<String, String>,
}

pub fn build_emitted_key_lookup(models: &[&SemanticModel]) -> EmittedKeyLookup {
    let mut emitted_keys = HashSet::new();
    let mut unique_keys_by_name = HashMap::new();
    let mut duplicated_names = HashSet::new();

    for model in models {
        for descriptor in &model.descriptors {
            let emitted_key = descriptor.key.clone();
            emitted_keys.insert(emitted_key.clone());
            if duplicated_names.contains(&descriptor.name) {
                continue;
            }
            if unique_keys_by_name.contains_key(&descriptor.name) {
                unique_keys_by_name.remove(&descriptor.name);
                duplicated_names.insert(descriptor.name.clone());
                continue;
            }
            unique_keys_by_name.insert(descriptor.name.clone(), emitted_key);
        }
    }

    EmittedKeyLookup { emitted_keys, unique_keys_by_name }
}

pub fn lower_schema_model_to_loader_ir(
    model: &SemanticModel,
    meta: LoaderMeta,
    emitted_key_lookup: &EmittedKeyLookup,
) -> LoaderDocument {
    let schema = model.schemas.first().expect("schema model has schema");
    let mut holons = Vec::new();
    if schema.header.is_some()
        || !schema.literal_properties.is_empty()
        || !schema.literal_relationships.is_empty()
    {
        holons.push(lower_schema_holon(schema));
    }
    for descriptor in &model.descriptors {
        holons.push(lower_descriptor_holon(descriptor, emitted_key_lookup));
    }

    LoaderDocument { meta, holons }
}

pub fn emit_loader_document_json(document: &LoaderDocument) -> Result<String> {
    render_loader_document(document)
}

pub fn schema_matches_semantic_loader_shape(schema: &Schema) -> bool {
    if schema.literal_properties.is_empty() && schema.literal_relationships.is_empty() {
        return true;
    }

    let properties_ok =
        schema.literal_properties.iter().all(|(key, _)| schema_literal_property_is_renderable(key));
    let relationships_ok = schema
        .literal_relationships
        .iter()
        .all(|relationship| schema_literal_relationship_is_renderable(&relationship.name));

    properties_ok && relationships_ok
}

pub fn descriptor_matches_semantic_loader_shape(
    descriptor: &TypeDescriptor,
    emitted_key_lookup: &EmittedKeyLookup,
) -> bool {
    if descriptor.literal_properties.is_empty() && descriptor.literal_relationships.is_empty() {
        return true;
    }

    let properties_ok = descriptor
        .literal_properties
        .iter()
        .all(|(key, _)| descriptor_literal_property_is_renderable(key));
    let relationships_ok = descriptor.literal_relationships.iter().all(|relationship| {
        descriptor_literal_relationship_is_renderable(
            &relationship.name,
            relationship.targets.as_slice(),
            emitted_key_lookup,
        )
    });

    properties_ok && relationships_ok
}

fn lower_schema_holon(schema: &Schema) -> LoaderHolon {
    if !schema.literal_properties.is_empty() || !schema.literal_relationships.is_empty() {
        return LoaderHolon {
            key: schema.name.clone(),
            descriptor_type: descriptor_type(&schema.described_by, "Schema.HolonType"),
            properties: if schema.literal_properties.is_empty() {
                let mut properties = serde_json::Map::new();
                properties.insert("schema_name".to_string(), json!(schema.name));
                properties
            } else {
                literal_object_to_json_map(&schema.literal_properties)
            },
            relationships: schema
                .literal_relationships
                .iter()
                .map(|relationship| {
                    loader_relationship(&relationship.name, relationship.targets.clone())
                })
                .collect(),
        };
    }

    lower_schema_holon_semantic(schema)
}

fn lower_schema_holon_semantic(schema: &Schema) -> LoaderHolon {
    let mut properties = serde_json::Map::new();
    if let Some(header) = &schema.header {
        if let Some(description) = &header.description {
            properties.insert("description".to_string(), json!(description));
        }
        if let Some(display_name) = &header.display_name {
            properties.insert("display_name".to_string(), json!(display_name));
        }
        if let Some(display_plural) = &header.display_name_plural {
            properties.insert("display_name_plural".to_string(), json!(display_plural));
        }
        if let Some(type_plural) = &header.type_name_plural {
            properties.insert("type_name_plural".to_string(), json!(type_plural));
        }
    }
    if !properties.contains_key("schema_name") {
        properties.insert("schema_name".to_string(), json!(schema.name));
    }
    if let Some(header) = &schema.header {
        if let Some(description) = &header.description {
            properties.entry("description".to_string()).or_insert(json!(description));
        }
    }
    let relationships = schema
        .dependencies
        .iter()
        .map(|dependency| loader_relationship("DependsOn", vec![dependency.target.clone()]))
        .collect();

    LoaderHolon {
        key: schema.name.clone(),
        descriptor_type: descriptor_type(&schema.described_by, "Schema.HolonType"),
        properties,
        relationships,
    }
}

fn lower_descriptor_holon(
    descriptor: &TypeDescriptor,
    emitted_key_lookup: &EmittedKeyLookup,
) -> LoaderHolon {
    if !descriptor.literal_properties.is_empty() || !descriptor.literal_relationships.is_empty() {
        return LoaderHolon {
            key: descriptor.key.clone(),
            descriptor_type: descriptor_type(&descriptor.described_by, "TypeDescriptor.HolonType"),
            properties: lower_descriptor_properties(descriptor),
            relationships: lower_descriptor_relationships(descriptor, emitted_key_lookup),
        };
    }

    lower_descriptor_holon_semantic(descriptor, emitted_key_lookup)
}

fn lower_descriptor_holon_semantic(
    descriptor: &TypeDescriptor,
    emitted_key_lookup: &EmittedKeyLookup,
) -> LoaderHolon {
    LoaderHolon {
        key: descriptor.key.clone(),
        descriptor_type: descriptor_type(&descriptor.described_by, "TypeDescriptor.HolonType"),
        properties: lower_descriptor_properties_semantic(descriptor),
        relationships: lower_descriptor_relationships_semantic(descriptor, emitted_key_lookup),
    }
}

fn descriptor_type(references: &[SemanticReference], fallback: &str) -> String {
    references
        .first()
        .map(|reference| reference.target.clone())
        .unwrap_or_else(|| fallback.to_string())
}

fn lower_descriptor_properties(descriptor: &TypeDescriptor) -> serde_json::Map<String, Value> {
    if !descriptor.literal_properties.is_empty() {
        let mut properties = literal_object_to_json_map(&descriptor.literal_properties);
        append_materialized_properties(&mut properties, descriptor);
        return properties;
    }

    lower_descriptor_properties_semantic(descriptor)
}

fn lower_descriptor_properties_semantic(
    descriptor: &TypeDescriptor,
) -> serde_json::Map<String, Value> {
    let mut properties = serde_json::Map::new();
    properties.insert("type_name".to_string(), json!(descriptor.name));
    if let Some(header) = &descriptor.header {
        if let Some(type_plural) = &header.type_name_plural {
            properties.insert("type_name_plural".to_string(), json!(type_plural));
        } else {
            properties
                .insert("type_name_plural".to_string(), json!(pluralize_name(&descriptor.name)));
        }
    } else {
        properties.insert("type_name_plural".to_string(), json!(pluralize_name(&descriptor.name)));
    }
    if let Some(header) = &descriptor.header {
        if let Some(display_name) = &header.display_name {
            properties.insert("display_name".to_string(), json!(display_name));
        } else {
            properties.insert("display_name".to_string(), json!(descriptor.name));
        }
        if let Some(display_plural) = &header.display_name_plural {
            properties.insert("display_name_plural".to_string(), json!(display_plural));
        }
        if let Some(description) = &header.description {
            properties.insert("description".to_string(), json!(description));
        }
    } else {
        properties.insert("display_name".to_string(), json!(descriptor.name));
        properties
            .insert("display_name_plural".to_string(), json!(pluralize_name(&descriptor.name)));
    }

    if let Some(instance_type_kind) = &descriptor.instance_type_kind {
        properties.insert("instance_type_kind".to_string(), json!(instance_type_kind));
    }
    properties.insert("is_abstract_type".to_string(), json!(descriptor.is_abstract));

    match descriptor.kind {
        DescriptorKind::HolonType | DescriptorKind::TypeDescriptor => {
            properties.insert(
                "allows_additional_properties".to_string(),
                json!(descriptor.allows_additional_properties),
            );
            properties.insert(
                "allows_additional_relationships".to_string(),
                json!(descriptor.allows_additional_relationships),
            );
        }
        DescriptorKind::RelationshipType => {
            if let Some(deletion_semantic) = &descriptor.deletion_semantic {
                properties.insert("deletion_semantic".to_string(), json!(deletion_semantic));
            }
            properties.insert("is_definitional".to_string(), json!(descriptor.is_definitional));
            properties.insert("is_ordered".to_string(), json!(descriptor.is_ordered));
            properties.insert("allows_duplicates".to_string(), json!(descriptor.allows_duplicates));
            if let Some(min) = descriptor.min_cardinality {
                properties.insert("min_cardinality".to_string(), json!(min));
            }
            if let Some(max) = descriptor.max_cardinality {
                properties.insert("max_cardinality".to_string(), json!(max));
            }
        }
        DescriptorKind::PropertyType => {
            if let Some(required) = descriptor.property_required {
                properties.insert("is_required".to_string(), json!(required));
            }
        }
        DescriptorKind::ValueType
        | DescriptorKind::Enum
        | DescriptorKind::EnumVariant
        | DescriptorKind::Schema => {}
    }
    append_materialized_properties(&mut properties, descriptor);
    properties
}

fn append_materialized_properties(
    properties: &mut serde_json::Map<String, Value>,
    descriptor: &TypeDescriptor,
) {
    for (name, value) in descriptor.materialized_properties.iter() {
        properties
            .entry(canonical_property_to_snake_case(name))
            .or_insert_with(|| literal_value_to_json(value));
    }
}

fn canonical_property_to_snake_case(name: &str) -> String {
    let characters = name.chars().collect::<Vec<_>>();
    let mut snake = String::with_capacity(name.len());
    for (index, character) in characters.iter().copied().enumerate() {
        let previous_is_lowercase = index
            .checked_sub(1)
            .and_then(|previous| characters.get(previous))
            .is_some_and(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit());
        let next_is_lowercase =
            characters.get(index + 1).is_some_and(|next| next.is_ascii_lowercase());
        if character.is_ascii_uppercase()
            && index > 0
            && (previous_is_lowercase || next_is_lowercase)
        {
            snake.push('_');
        }
        snake.push(character.to_ascii_lowercase());
    }
    snake
}

fn lower_descriptor_relationships(
    descriptor: &TypeDescriptor,
    emitted_key_lookup: &EmittedKeyLookup,
) -> Vec<LoaderRelationship> {
    if !descriptor.literal_relationships.is_empty() {
        return descriptor
            .literal_relationships
            .iter()
            .filter(|relationship| relationship.name != "InverseOf")
            .map(|relationship| {
                loader_relationship(
                    &relationship.name,
                    relationship
                        .targets
                        .iter()
                        .map(|target| {
                            normalize_emitted_reference_target(target, emitted_key_lookup)
                        })
                        .collect(),
                )
            })
            .collect();
    }

    lower_descriptor_relationships_semantic(descriptor, emitted_key_lookup)
}

fn lower_descriptor_relationships_semantic(
    descriptor: &TypeDescriptor,
    emitted_key_lookup: &EmittedKeyLookup,
) -> Vec<LoaderRelationship> {
    let mut relationships = Vec::new();
    relationships.push(loader_relationship("ComponentOf", vec![descriptor.owning_schema.clone()]));

    if let Some(extends) = &descriptor.extends {
        relationships.push(loader_relationship(
            "Extends",
            vec![normalize_emitted_reference_target(&extends.target, emitted_key_lookup)],
        ));
    }
    if let Some(has_inverse) = &descriptor.has_inverse {
        relationships.push(loader_relationship(
            "HasInverse",
            vec![normalize_emitted_reference_target(&has_inverse.target, emitted_key_lookup)],
        ));
    }
    if let Some(value_type) = &descriptor.value_type {
        relationships.push(loader_relationship(
            "ValueType",
            vec![normalize_emitted_reference_target(&value_type.target, emitted_key_lookup)],
        ));
    }
    if let Some(source_type) = &descriptor.source_type {
        relationships.push(loader_relationship(
            "SourceType",
            vec![normalize_emitted_reference_target(&source_type.target, emitted_key_lookup)],
        ));
    }
    if let Some(target_type) = &descriptor.target_type {
        relationships.push(loader_relationship(
            "TargetType",
            vec![normalize_emitted_reference_target(&target_type.target, emitted_key_lookup)],
        ));
    }
    if let Some(key_rule) = &descriptor.key_rule {
        relationships.push(loader_relationship(
            "UsesKeyRule",
            vec![normalize_emitted_reference_target(&key_rule.target, emitted_key_lookup)],
        ));
    }
    if let Some(parent) = &descriptor.variant_of {
        if descriptor.kind != DescriptorKind::EnumVariant {
            relationships.push(loader_relationship(
                "VariantOf",
                vec![normalize_emitted_reference_target(&parent.target, emitted_key_lookup)],
            ));
        }
    }

    if !descriptor.instance_properties.is_empty() {
        relationships.push(loader_relationship(
            "InstanceProperties",
            descriptor
                .instance_properties
                .iter()
                .map(|property| {
                    normalize_emitted_reference_target(&property.target, emitted_key_lookup)
                })
                .collect(),
        ));
    }
    if !descriptor.instance_relationships.is_empty() {
        relationships.push(loader_relationship(
            "InstanceRelationships",
            descriptor
                .instance_relationships
                .iter()
                .map(|relationship| {
                    normalize_emitted_reference_target(&relationship.target, emitted_key_lookup)
                })
                .collect(),
        ));
    }

    if descriptor.kind == DescriptorKind::Enum {
        let variants = descriptor
            .variants
            .iter()
            .map(|variant| normalize_emitted_reference_target(&variant.target, emitted_key_lookup))
            .collect::<Vec<_>>();
        if !variants.is_empty() {
            relationships.push(loader_relationship("Variants", variants));
        }
    }

    relationships
}

fn normalize_emitted_reference_target(
    target: &str,
    emitted_key_lookup: &EmittedKeyLookup,
) -> String {
    if emitted_key_lookup.emitted_keys.contains(target) {
        return target.to_string();
    }
    emitted_key_lookup
        .unique_keys_by_name
        .get(target)
        .cloned()
        .unwrap_or_else(|| target.to_string())
}

fn schema_literal_property_is_renderable(key: &str) -> bool {
    matches!(
        key,
        "schema_name"
            | "description"
            | "display_name"
            | "display_name_plural"
            | "type_name_plural"
            | "allows_additional_properties"
            | "allows_additional_relationships"
    )
}

fn schema_literal_relationship_is_renderable(name: &str) -> bool {
    name == "DependsOn"
}

fn descriptor_literal_property_is_renderable(key: &str) -> bool {
    matches!(
        key,
        "type_name"
            | "type_name_plural"
            | "display_name"
            | "display_name_plural"
            | "description"
            | "instance_type_kind"
            | "is_abstract_type"
            | "allows_additional_properties"
            | "allows_additional_relationships"
            | "is_definitional"
            | "is_ordered"
            | "allows_duplicates"
            | "is_required"
            | "arity"
            | "template_string"
            | "min_cardinality"
            | "max_cardinality"
            | "deletion_semantic"
    )
}

fn descriptor_literal_relationship_is_renderable(
    name: &str,
    targets: &[String],
    emitted_key_lookup: &EmittedKeyLookup,
) -> bool {
    if targets.iter().any(|target| target.is_empty()) {
        return false;
    }

    match name {
        "ComponentOf"
        | "Extends"
        | "UsesKeyRule"
        | "SourceType"
        | "TargetType"
        | "InverseOf"
        | "HasInverse"
        | "ValueType"
        | "VariantOf"
        | "InstanceProperties"
        | "InstanceRelationships"
        | "Variants" => targets.iter().all(|target| {
            let normalized = normalize_emitted_reference_target(target, emitted_key_lookup);
            !normalized.is_empty()
        }),
        _ => false,
    }
}

fn loader_relationship(name: &str, targets: Vec<String>) -> LoaderRelationship {
    LoaderRelationship {
        name: name.to_string(),
        targets: targets.into_iter().map(|target| LoaderReference { target }).collect(),
    }
}

fn render_loader_document(document: &LoaderDocument) -> Result<String> {
    let mut out = String::new();
    out.push_str("{\n");
    write_json_field(&mut out, 1, "meta", &loader_meta_json(&document.meta))?;
    out.push_str(",\n");
    out.push_str(&format!("{}\"holons\": [\n", INDENT));
    for (index, holon) in document.holons.iter().enumerate() {
        out.push_str(&render_loader_holon(holon, 2)?);
        if index + 1 != document.holons.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
    out.push_str("  ]\n");
    out.push('}');
    Ok(out)
}

fn loader_meta_json(meta: &LoaderMeta) -> Value {
    let mut object = Map::new();
    if let Some(generator) = &meta.generator {
        object.insert("generator".to_string(), json!(generator));
    }
    if let Some(generated_at) = &meta.generated_at {
        object.insert("generated_at".to_string(), json!(generated_at));
    }
    if let Some(export_mode) = &meta.export_mode {
        object.insert("export_mode".to_string(), json!(export_mode));
    }
    object.insert("source_files".to_string(), json!(meta.source_files));
    if !meta.load_with.is_empty() {
        object.insert("load_with".to_string(), json!(meta.load_with));
    }
    Value::Object(object)
}

fn render_loader_holon(holon: &LoaderHolon, indent_level: usize) -> Result<String> {
    let mut out = String::new();
    let indent = INDENT.repeat(indent_level);
    out.push_str(&format!("{}{{\n", indent));
    write_json_field(&mut out, indent_level + 1, "key", &json!(holon.key))?;
    out.push_str(",\n");
    write_json_field(&mut out, indent_level + 1, "type", &json!(holon.descriptor_type))?;
    out.push_str(",\n");
    write_json_field(
        &mut out,
        indent_level + 1,
        "properties",
        &Value::Object(holon.properties.clone().into_iter().collect()),
    )?;
    if !holon.relationships.is_empty() {
        out.push_str(",\n");
        write_json_field(
            &mut out,
            indent_level + 1,
            "relationships",
            &loader_relationships_json(&holon.relationships),
        )?;
    }
    out.push('\n');
    out.push_str(&format!("{}}}", indent));
    Ok(out)
}

fn loader_relationships_json(relationships: &[LoaderRelationship]) -> Value {
    Value::Array(relationships.iter().map(loader_relationship_json).collect())
}

fn loader_relationship_json(relationship: &LoaderRelationship) -> Value {
    let mut object = Map::new();
    object.insert("name".to_string(), json!(relationship.name));
    if relationship.targets.len() == 1 {
        object.insert("target".to_string(), json!({ "$ref": relationship.targets[0].target }));
    } else {
        object.insert(
            "target".to_string(),
            Value::Array(
                relationship
                    .targets
                    .iter()
                    .map(|target| json!({ "$ref": target.target }))
                    .collect(),
            ),
        );
    }
    Value::Object(object)
}

fn write_json_field(
    out: &mut String,
    indent_level: usize,
    name: &str,
    value: &Value,
) -> Result<()> {
    let rendered = serde_json::to_string_pretty(value)?;
    let indent = INDENT.repeat(indent_level);
    let mut lines = rendered.lines();
    if let Some(first) = lines.next() {
        out.push_str(&format!("{}\"{}\": {}", indent, name, first));
        for line in lines {
            out.push('\n');
            out.push_str(&indent);
            out.push_str(line);
        }
    }
    Ok(())
}

fn pluralize_name(name: &str) -> String {
    if name.ends_with('s') {
        format!("{name}es")
    } else {
        format!("{name}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_ir::{
        LiteralRelationship, Origin, ReferenceRole, RelationshipFlavor, SourceKind,
    };

    #[test]
    fn loader_projection_emits_only_the_declared_side_of_an_inverse_pair() {
        let declared_key = "(Book.HolonType)-[WrittenBy]->(Person.HolonType)";
        let inverse_key = "(Person.HolonType)-[AuthorOf]->(Book.HolonType)";

        let mut declared = TypeDescriptor::new(
            declared_key,
            "WrittenBy",
            DescriptorKind::RelationshipType,
            "TestSchema",
            Origin::new(SourceKind::TdlSource),
        );
        declared.relationship_flavor = Some(RelationshipFlavor::Declared);
        declared.has_inverse =
            Some(SemanticReference::unresolved(ReferenceRole::HasInverse, inverse_key));

        let mut inverse = TypeDescriptor::new(
            inverse_key,
            "AuthorOf",
            DescriptorKind::RelationshipType,
            "TestSchema",
            Origin::new(SourceKind::TdlSource),
        );
        inverse.relationship_flavor = Some(RelationshipFlavor::Inverse);
        inverse.inverse_of =
            Some(SemanticReference::unresolved(ReferenceRole::InverseOf, declared_key));

        let emitted_key_lookup = EmittedKeyLookup::default();
        let declared_relationships = lower_descriptor_relationships(&declared, &emitted_key_lookup);
        let semantic_inverse_relationships =
            lower_descriptor_relationships(&inverse, &emitted_key_lookup);
        inverse.literal_relationships.push(LiteralRelationship {
            name: "InverseOf".to_string(),
            targets: vec![declared_key.to_string()],
        });
        let literal_inverse_relationships =
            lower_descriptor_relationships(&inverse, &emitted_key_lookup);

        assert!(declared_relationships
            .iter()
            .any(|relationship| relationship.name == "HasInverse"));
        assert!(!declared_relationships
            .iter()
            .chain(semantic_inverse_relationships.iter())
            .chain(literal_inverse_relationships.iter())
            .any(|relationship| relationship.name == "InverseOf"));
    }
}
