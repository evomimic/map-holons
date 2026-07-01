use crate::{
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

    let semantic = lower_schema_holon_semantic(schema);
    json_map_matches_in_order(&schema.literal_properties, &semantic.properties)
        && loader_relationships_match_in_order(
            &schema
                .literal_relationships
                .iter()
                .map(|relationship| loader_relationship(&relationship.name, relationship.targets.clone()))
                .collect::<Vec<_>>(),
            &semantic.relationships,
        )
}

pub fn descriptor_matches_semantic_loader_shape(
    descriptor: &TypeDescriptor,
    emitted_key_lookup: &EmittedKeyLookup,
) -> bool {
    if descriptor.literal_properties.is_empty() && descriptor.literal_relationships.is_empty() {
        return true;
    }

    let semantic = lower_descriptor_holon_semantic(descriptor, emitted_key_lookup);
    json_map_matches_in_order(&descriptor.literal_properties, &semantic.properties)
        && loader_relationships_match_in_order(
            &descriptor
                .literal_relationships
                .iter()
                .map(|relationship| {
                    loader_relationship(
                        &relationship.name,
                        relationship
                            .targets
                            .iter()
                            .map(|target| normalize_emitted_reference_target(target, emitted_key_lookup))
                            .collect(),
                    )
                })
                .collect::<Vec<_>>(),
            &semantic.relationships,
        )
}

fn lower_schema_holon(schema: &Schema) -> LoaderHolon {
    if !schema.literal_properties.is_empty() || !schema.literal_relationships.is_empty() {
        return LoaderHolon {
            key: schema.name.clone(),
            descriptor_type: "Schema.HolonType".to_string(),
            properties: if schema.literal_properties.is_empty() {
                let mut properties = serde_json::Map::new();
                properties.insert("schema_name".to_string(), json!(schema.name));
                properties
            } else {
                schema.literal_properties.clone()
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
        descriptor_type: "Schema.HolonType".to_string(),
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
            descriptor_type: "TypeDescriptor.HolonType".to_string(),
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
        descriptor_type: "TypeDescriptor.HolonType".to_string(),
        properties: lower_descriptor_properties_semantic(descriptor),
        relationships: lower_descriptor_relationships_semantic(descriptor, emitted_key_lookup),
    }
}

fn lower_descriptor_properties(
    descriptor: &TypeDescriptor,
) -> serde_json::Map<String, Value> {
    if !descriptor.literal_properties.is_empty() {
        return descriptor.literal_properties.clone();
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

    let instance_type_kind = match descriptor.kind {
        DescriptorKind::HolonType => Some(json!("TypeKind.Holon")),
        DescriptorKind::PropertyType => Some(json!("TypeKind.Property")),
        DescriptorKind::RelationshipType => Some(json!("TypeKind.Relationship")),
        DescriptorKind::ValueType => Some(json!(infer_value_kind(descriptor))),
        DescriptorKind::Enum => Some(json!("TypeKind.Value.Enum")),
        DescriptorKind::EnumVariant => Some(json!("TypeKind.EnumVariant")),
        DescriptorKind::TypeDescriptor => Some(json!("TypeKind.Holon")),
        DescriptorKind::Schema => None,
    };
    if let Some(instance_type_kind) = instance_type_kind {
        properties.insert("instance_type_kind".to_string(), instance_type_kind);
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
        DescriptorKind::PropertyType
        | DescriptorKind::ValueType
        | DescriptorKind::Enum
        | DescriptorKind::EnumVariant
        | DescriptorKind::Schema => {}
    }
    properties
}

fn infer_value_kind(descriptor: &TypeDescriptor) -> &'static str {
    if matches!(descriptor.name.as_str(), "MetaValueType" | "ValueType") {
        return "TypeKind.Holon";
    }

    if matches!(descriptor.name.as_str(), "EnumVariantValueType" | "MapEnumVariantValueType") {
        return "TypeKind.EnumVariant";
    }

    if descriptor.name.ends_with("StringValueType") {
        return "TypeKind.Value.String";
    }
    if descriptor.name.ends_with("IntegerValueType") {
        return "TypeKind.Value.Integer";
    }
    if descriptor.name.ends_with("BooleanValueType") {
        return "TypeKind.Value.Boolean";
    }
    if descriptor.name.ends_with("BytesValueType") || descriptor.name == "HolonIdValueType" {
        return "TypeKind.Value.Bytes";
    }
    if descriptor.name.ends_with("ValueArrayType")
        || descriptor.name.ends_with("ValueArrayValueType")
    {
        return "TypeKind.Value.Array";
    }
    if descriptor.name.ends_with("EnumValueType") {
        return "TypeKind.Value.Enum";
    }

    if let Some(extends) = descriptor.extends.as_ref().map(|extends| extends.target.as_str()) {
        match extends {
            "MetaTypeDescriptor" | "MetaValueType" => "TypeKind.Holon",
            "StringValueType" | "MapStringValueType" => "TypeKind.Value.String",
            "IntegerValueType" | "MapIntegerValueType" => "TypeKind.Value.Integer",
            "BooleanValueType" | "MapBooleanValueType" => "TypeKind.Value.Boolean",
            "BytesValueType" | "MapBytesValueType" | "HolonIdValueType" => "TypeKind.Value.Bytes",
            "ValueArrayValueType" | "MapValueArrayType" => "TypeKind.Value.Array",
            "EnumValueType" | "MapEnumValueType" => "TypeKind.Value.Enum",
            "EnumVariantValueType" | "MapEnumVariantValueType" => "TypeKind.EnumVariant",
            _ => "TypeKind.Value.String",
        }
    } else {
        "TypeKind.Value.String"
    }
}

fn lower_descriptor_relationships(
    descriptor: &TypeDescriptor,
    emitted_key_lookup: &EmittedKeyLookup,
) -> Vec<LoaderRelationship> {
    if !descriptor.literal_relationships.is_empty() {
        return descriptor
            .literal_relationships
            .iter()
            .map(|relationship| {
                loader_relationship(
                    &relationship.name,
                    relationship
                        .targets
                        .iter()
                        .map(|target| normalize_emitted_reference_target(target, emitted_key_lookup))
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
    relationships.push(loader_relationship(
        "ComponentOf",
        vec![descriptor.owning_schema.clone()],
    ));

    if let Some(extends) = &descriptor.extends {
        relationships.push(loader_relationship(
            "Extends",
            vec![normalize_emitted_reference_target(&extends.target, emitted_key_lookup)],
        ));
    }
    if let Some(inverse_of) = &descriptor.inverse_of {
        relationships.push(loader_relationship(
            "InverseOf",
            vec![normalize_inverse_reference_target(descriptor, inverse_of, emitted_key_lookup)],
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
        relationships.push(loader_relationship(
            "VariantOf",
            vec![normalize_emitted_reference_target(&parent.target, emitted_key_lookup)],
        ));
    }

    if !descriptor.instance_properties.is_empty() {
        relationships.push(loader_relationship(
            "InstanceProperties",
            descriptor
                .instance_properties
                .iter()
                .map(|property| normalize_emitted_reference_target(&property.target, emitted_key_lookup))
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
            .instance_relationships
            .iter()
            .filter(|relationship| relationship.target.starts_with(&descriptor.name))
            .map(|relationship| {
                normalize_emitted_reference_target(&relationship.target, emitted_key_lookup)
            })
            .collect::<Vec<_>>();
        if !variants.is_empty() {
            relationships.push(loader_relationship("Variants", variants));
        }
    }

    relationships
}

fn json_map_matches_in_order(expected: &Map<String, Value>, actual: &Map<String, Value>) -> bool {
    expected.len() == actual.len()
        && expected.keys().eq(actual.keys())
        && expected
            .iter()
            .zip(actual.iter())
            .all(|((expected_key, expected_value), (actual_key, actual_value))| {
                expected_key == actual_key && expected_value == actual_value
            })
}

fn loader_relationships_match_in_order(
    expected: &[LoaderRelationship],
    actual: &[LoaderRelationship],
) -> bool {
    expected.len() == actual.len()
        && expected.iter().zip(actual.iter()).all(|(expected_relationship, actual_relationship)| {
            expected_relationship.name == actual_relationship.name
                && expected_relationship.targets.len() == actual_relationship.targets.len()
                && expected_relationship
                    .targets
                    .iter()
                    .zip(actual_relationship.targets.iter())
                    .all(|(expected_target, actual_target)| {
                        expected_target.target == actual_target.target
                    })
        })
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

fn normalize_inverse_reference_target(
    descriptor: &TypeDescriptor,
    inverse_of: &SemanticReference,
    emitted_key_lookup: &EmittedKeyLookup,
) -> String {
    let inverse_target = &inverse_of.target;
    if inverse_target.contains(")-[") {
        return normalize_emitted_reference_target(inverse_target, emitted_key_lookup);
    }

    match (
        descriptor.relationship_flavor,
        descriptor.source_type.as_ref(),
        descriptor.target_type.as_ref(),
    ) {
        (Some(crate::schema_ir::RelationshipFlavor::Inverse), Some(source_type), Some(target_type)) => {
            format!(
                "({})-[{}]->({})",
                target_type.target, inverse_target, source_type.target
            )
        }
        _ => emitted_key_lookup
            .unique_keys_by_name
            .get(inverse_target)
            .cloned()
            .unwrap_or_else(|| normalize_emitted_reference_target(inverse_target, emitted_key_lookup)),
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
        object.insert(
            "target".to_string(),
            json!({ "$ref": relationship.targets[0].target }),
        );
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
