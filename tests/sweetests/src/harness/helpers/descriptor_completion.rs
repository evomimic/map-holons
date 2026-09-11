use core_types::ContentSet;
use holons_prelude::prelude::*;
use std::collections::BTreeSet;

/// Captures descriptor identities from the input, independently of runtime
/// descriptor resolution, so deferred completion cannot make the assertion vacuous.
pub fn expected_descriptor_keys(content_set: &ContentSet) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for file in &content_set.files_to_load {
        let document: serde_json::Value = serde_json::from_str(&file.raw_contents)
            .unwrap_or_else(|error| panic!("invalid schema JSON in {}: {error}", file.filename));
        let holons = document["holons"].as_array().expect("schema must contain holons");
        for holon in holons {
            // Descriptor-oriented TDL declarations compile with an implicit
            // ComponentOf edge. This distinguishes them from ordinary instances
            // whose domain type may itself begin with `Meta` (for example, a
            // MetaDesignSystem instance).
            let is_descriptor = holon["relationships"].as_array().is_some_and(|relationships| {
                relationships
                    .iter()
                    .any(|relationship| relationship["name"].as_str() == Some("ComponentOf"))
            });
            if is_descriptor {
                keys.insert(holon["key"].as_str().expect("descriptor must have a key").to_owned());
            }
        }
    }
    assert!(!keys.is_empty(), "schema completion check must include descriptors");
    keys
}

/// Checks explicit descriptor defaults on the returned staged pool after load.
/// Input-derived identities also ensure every expected descriptor was inspected.
pub fn assert_descriptor_completion(
    context: &TransactionContext,
    mut expected_keys: BTreeSet<String>,
) {
    for staged in context.staged_references().expect("failed to read schema staged pool") {
        let Some(key) = staged.key().expect("failed to read staged schema key") else {
            continue;
        };
        if !expected_keys.remove(&key.0) {
            continue;
        }
        for property in ["IsAbstractType", "DefinesInstanceTypeKind"] {
            let value =
                staged.property_value(property).expect("failed to read descriptor property");
            assert!(
                matches!(value, Some(BaseValue::BooleanValue(_))),
                "descriptor {} must have explicit {:?} after schema completion; got {:?}",
                key.0,
                property,
                value
            );
        }
    }
    assert!(
        expected_keys.is_empty(),
        "schema descriptors missing from staged pool: {expected_keys:?}"
    );
}
