//! Exercise policy binding against committed Core schema, including meta-types.

use holons_core::{HolonReference, ReadableHolon, RelationshipReadHint};
use holons_prelude::prelude::MapString;
use holons_test::{init_test_runtime, DancesTestCase};
use std::sync::Arc;
use std::time::Instant;

#[tokio::test(flavor = "multi_thread")]
async fn configured_inverse_policies_retain_committed_membership() {
    let mut test_case = DancesTestCase::default();
    let started = Instant::now();
    let (runtime, transaction_id) = init_test_runtime(&mut test_case).await;
    eprintln!("cache-policy bootstrap completed in {:?}", started.elapsed());
    let context = runtime.session().get_transaction(&transaction_id).unwrap();

    for (key, name) in [
        ("HolonType.TypeDescriptor", "AffordsDance"),
        ("TypeName.PropertyType", "HasApplicableVisualizer"),
        ("MaterializeVisualizer.DanceType", "HasImplementation"),
    ] {
        let started = Instant::now();
        let source = HolonReference::Smart(
            context.lookup().get_saved_holon_by_key(&MapString::from(key)).unwrap(),
        );
        let relationship =
            source.holon_descriptor().unwrap().resolve_available_relationship(name).unwrap();
        assert_eq!(
            relationship.descriptor.membership_cache_max_age_millis().unwrap(),
            30_000,
            "{key}.{name} must bind its own configured policy"
        );
        let first = source.related_holons(name).unwrap();
        assert!(
            Arc::ptr_eq(&first, &source.related_holons(name).unwrap()),
            "{key}.{name} must reuse membership within its TTL"
        );
        let fresh =
            source.related_holons_with_hint(name, RelationshipReadHint::RequireFresh).unwrap();
        assert!(!Arc::ptr_eq(&first, &fresh), "explicit fresh must fetch again");
        assert!(Arc::ptr_eq(&fresh, &source.related_holons(name).unwrap()));
        eprintln!("verified {key}.{name} in {:?}", started.elapsed());
    }
    runtime.session().archive_transaction(&transaction_id).unwrap();
}
