//! Bootstrap schemas must be discoverable through ordinary Space navigation.

use holons_core::ReadableHolon;
use holons_test::{init_test_runtime, DancesTestCase};
use std::collections::BTreeSet;

#[tokio::test(flavor = "multi_thread")]
async fn bootstrap_space_exposes_selected_schemas_and_committed_inverses() {
    let mut test_case = DancesTestCase::default();
    let (runtime, transaction_id) = init_test_runtime(&mut test_case).await;
    let context = runtime.session().get_transaction(&transaction_id).unwrap();
    let space = context.get_space_holon().unwrap().expect("bootstrap Space");

    assert!(space.available_relationships().unwrap().iter().any(|relationship| {
        relationship.descriptor.base_relationship_name().unwrap().to_string() == "AvailableSchemas"
    }));
    let schemas =
        space.related_holons("AvailableSchemas").unwrap().read().unwrap().get_members().clone();
    let expected = BTreeSet::from([
        "MAP Commands Schema-v0.1.0",
        "MAP Core Schema-v0.0.7",
        "MAP DAHN Schema-v0.1.0",
        "MAP Dance Schema-v0.1.0",
        "MAP Dancer Schema-v0.1.0",
        "MAP Design Tokens Schema-v0.1.0",
        "MAP Meta Design System Schema-v0.1.0",
        "MAP Query Dance Adapter Schema-v0.1.0",
        "MAP Query Schema-v0.0.2",
        "MAP Theme Schema-v0.1.0",
        "MAP Validation Schema-v0.1.0",
    ]);
    let keys: BTreeSet<String> =
        schemas.iter().map(|schema| schema.key().unwrap().unwrap().to_string()).collect();
    assert_eq!(keys, expected.into_iter().map(str::to_owned).collect());
    assert_eq!(schemas.len(), keys.len(), "schemas must not be duplicated");
    for schema in schemas {
        assert!(schema.available_relationships().unwrap().iter().any(|relationship| {
            relationship.descriptor.base_relationship_name().unwrap().to_string()
                == "AvailableInSpace"
        }));
        let spaces = schema
            .related_holons("AvailableInSpace")
            .unwrap()
            .read()
            .unwrap()
            .get_members()
            .clone();
        assert_eq!(spaces.len(), 1);
        assert_eq!(spaces[0].holon_id(), space.holon_id());
    }
    runtime.session().archive_transaction(&transaction_id).unwrap();
}
