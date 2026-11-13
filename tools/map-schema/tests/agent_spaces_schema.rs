//! The extension stays independent of Dancers and preserves its approved descriptor contracts.
use anyhow::Result;
use map_schema_tool::tdl_compiler::compile_input_string;
use serde_json::Value;
use std::{fs, path::PathBuf};

const SOURCE: &str = include_str!("../../../schema-src/agent-spaces/schema.tdl");
const GENERATED: &str = include_str!("../../../generated/json-imports/agent-spaces/schema.json");

fn holon<'a>(document: &'a Value, key: &str) -> &'a Value {
    document["holons"].as_array().unwrap().iter().find(|h| h["key"] == key).unwrap()
}

fn targets(holon: &Value, name: &str) -> Vec<String> {
    holon["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["name"] == name)
        .flat_map(|r| r["target"].as_array().unwrap())
        .map(|r| r["$ref"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn agent_contract_compiles_without_core_or_dancer_changes() -> Result<()> {
    let document: Value = serde_json::from_str(&compile_input_string(SOURCE, "schema.tdl")?)?;
    assert_eq!(document, serde_json::from_str::<Value>(GENERATED)?);
    assert_eq!(
        targets(holon(&document, "MAP Agent Spaces Schema-v0.1.0"), "DependsOn"),
        ["MAP Core Schema-v0.0.7"]
    );
    for (key, parent, abstract_type) in [
        ("Agent.HolonType", "HolonType.TypeDescriptor", true),
        ("BioSocialAgent.Agent", "Agent.HolonType", true),
        ("PersonAgent.BioSocialAgent", "BioSocialAgent.Agent", false),
    ] {
        let descriptor = holon(&document, key);
        assert_eq!(descriptor["type"], "MetaHolonType.MetaTypeDescriptor");
        assert_eq!(targets(descriptor, "Extends"), [parent]);
        assert_eq!(
            descriptor["properties"]["IsAbstractType"].as_bool().unwrap_or(false),
            abstract_type
        );
        assert!(descriptor["properties"].get("DefinesInstanceTypeKind").is_none());
    }
    for (key, required) in [("FirstName", false), ("MiddleName", false), ("LastName", true)] {
        let property = holon(&document, &format!("{key}.PropertyType"));
        assert_eq!(property["properties"]["IsValueRequired"], required);
        assert!(property["properties"].get("DefaultValue").is_none());
        assert_eq!(targets(property, "ValueType"), ["MapStringValueType.StringValueType"]);
    }
    let person = holon(&document, "PersonAgent.BioSocialAgent");
    assert_eq!(targets(person, "InstanceProperties").len(), 3);
    assert_eq!(targets(person, "InstanceKeyRule"), ["PersonAgentName.FormatRule"]);
    let rule = holon(&document, "PersonAgentName.FormatRule");
    assert_eq!(rule["properties"]["TemplateString"], "{0}.[1].[2]");
    assert_eq!(
        targets(rule, "TemplateParameters"),
        ["LastName.PropertyType", "FirstName.PropertyType", "MiddleName.PropertyType"]
    );
    assert_eq!(targets(rule, "RuleOf"), ["MAP Agent Spaces Schema-v0.1.0"]);
    Ok(())
}

#[test]
fn sovereign_relationships_have_asymmetric_contracts_and_only_declared_licensing() -> Result<()> {
    let document: Value = serde_json::from_str(GENERATED)?;
    let forward_key = "(BioSocialAgent.Agent)-[HasInterior]->(HolonSpace.HolonType)";
    let inverse_key = "(HolonSpace.HolonType)-[IsInteriorOf]->(BioSocialAgent.Agent)";
    let forward = holon(&document, forward_key);
    let inverse = holon(&document, inverse_key);
    assert_eq!(
        targets(holon(&document, "BioSocialAgent.Agent"), "InstanceRelationships"),
        [forward_key]
    );
    assert_eq!(targets(forward, "HasInverse"), [inverse_key]);
    assert!(targets(inverse, "InverseOf").is_empty());
    for (relationship, binding, definitional, deletion, constraint) in [
        (forward, "Version", true, "Block", "ExactlyOne.CardinalityConstraint"),
        (inverse, "Lineage", false, "Allow", "ZeroOrOne.CardinalityConstraint"),
    ] {
        let properties = &relationship["properties"];
        assert_eq!(properties["TargetBinding"], binding);
        assert_eq!(properties["IsDefinitional"], definitional);
        assert_eq!(properties["DeletionSemantic"], deletion);
        assert_eq!(properties["IsOrdered"], false);
        assert_eq!(properties["AllowsDuplicates"], false);
        assert_eq!(targets(relationship, "Constraints"), [constraint]);
    }
    assert!(forward["properties"].get("MembershipCacheMaxAgeMillis").is_none());
    assert_eq!(inverse["properties"]["MembershipCacheMaxAgeMillis"], 86_400_000);
    assert_eq!(targets(forward, "SourceType"), targets(inverse, "TargetType"));
    assert_eq!(targets(forward, "TargetType"), targets(inverse, "SourceType"));
    Ok(())
}

#[test]
fn configured_agent_schema_is_bundled_and_precedes_its_consumers() -> Result<()> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let resources = repository.join("host/conductora/resources");
    let config: Value = serde_json::from_slice(&fs::read(resources.join("space-imports.json"))?)?;
    let imports = config["imports"].as_array().unwrap();
    assert_eq!(imports[0], "schema-packages/agent-spaces/imports/schema.json");
    for import in imports {
        assert!(resources.join(import.as_str().unwrap()).is_file());
    }
    assert_eq!(GENERATED, fs::read_to_string(resources.join(imports[0].as_str().unwrap()))?);
    Ok(())
}
