//! Public DAHN descriptor contracts, including the scope distinction that prevents accidental broadening.
use anyhow::Result;
use map_schema_tool::tdl_compiler::compile_input_string;
use serde_json::Value;
const SOURCE: &str = include_str!("../../../schema-src/dahn/schema.tdl");
const GENERATED: &str = include_str!("../../../generated/json-imports/dahn/schema.json");
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
fn general_preference_has_independent_type_optional_scope_and_optional_configuration() -> Result<()>
{
    let document: Value = serde_json::from_str(&compile_input_string(SOURCE, "schema.tdl")?)?;
    assert_eq!(document, serde_json::from_str::<Value>(GENERATED)?);
    assert_eq!(
        targets(holon(&document, "VisualizerPreference.HolonType"), "Extends"),
        ["HolonType.TypeDescriptor"]
    );
    for (name, target, cardinality, binding) in [
        ("PreferenceForAgent", "PersonAgent.BioSocialAgent", "ExactlyOne", "Lineage"),
        ("PreferenceForSubjectType", "TypeDescriptor", "ExactlyOne", "Version"),
        ("PreferenceForSlot", "VisualizerSlot.HolonType", "ZeroOrOne", "Lineage"),
        ("PreferredVisualizer", "Visualizer.HolonType", "ExactlyOne", "Version"),
        ("PreferredUsage", "VisualizerUsage.HolonType", "ZeroOrOne", "Lineage"),
    ] {
        let relation =
            holon(&document, &format!("(VisualizerPreference.HolonType)-[{name}]->({target})"));
        assert_eq!(
            targets(relation, "Constraints"),
            [format!("{cardinality}.CardinalityConstraint")]
        );
        assert_eq!(relation["properties"]["TargetBinding"], binding);
        assert_eq!(targets(relation, "HasInverse").len(), 1);
    }
    // Old records cannot inherit the new optional-Slot interpretation.
    let old = holon(
        &document,
        "(VisualizerSlotPreference.HolonType)-[PreferenceForSlot]->(VisualizerSlot.HolonType)",
    );
    assert_eq!(targets(old, "Constraints"), ["ExactlyOne.CardinalityConstraint"]);
    Ok(())
}
#[test]
fn composition_grammar_is_typed_and_property_map_owns_direct_label_value_slots() -> Result<()> {
    let document: Value = serde_json::from_str(GENERATED)?;
    for name in ["CompositionSubject", "CompositionActivation", "VisualizerKind"] {
        assert_eq!(
            targets(holon(&document, &format!("{name}.PropertyType")), "ValueType"),
            [format!("{name}.MapEnumValueType")]
        );
    }
    assert_eq!(
        targets(
            holon(&document, "DefaultPropertyMapVisualizer.PropertyMapVisualizerType"),
            "HasSlot"
        ),
        ["DefaultPropertyMapVisualizer.LabelSlot", "DefaultPropertyMapVisualizer.ValueSlot"]
    );
    assert_eq!(
        holon(
            &document,
            "(ResolvedComposition.Projection)-[CompositionReferences]->(HolonType.TypeDescriptor)"
        )["properties"]["IsOrdered"],
        true
    );
    for (slot, binding) in [("Label", "ScalarPropertyLabel"), ("Value", "ScalarPropertyValue")] {
        let slot = holon(&document, &format!("DefaultPropertyMapVisualizer.{slot}Slot"));
        assert_eq!(slot["properties"]["CompositionSubject"], binding);
        assert_eq!(slot["properties"]["CompositionActivation"], "Presentation");
        assert_eq!(targets(slot, "AcceptsVisualizerType"), ["ValueVisualizer.HolonType"]);
    }
    assert_eq!(
        holon(&document, "HolonInspector.CollectionsSlot")["properties"]["CompositionActivation"],
        "Demand"
    );
    Ok(())
}
