use holons_prelude::prelude::*;
use holons_test::{ResolveBy, TestExecutionState, TestReference};
use pretty_assertions::assert_eq;
use tracing::info;

fn reference_identity(reference: &HolonReference) -> Result<String, HolonError> {
    let key = reference.key()?;
    let holon_id = reference.holon_id()?;

    Ok(match key {
        Some(key) => format!("key:{}|id:{}", key.0, holon_id),
        None => format!("id:{}", holon_id),
    })
}

pub async fn execute_assert_related_holons(
    state: &mut TestExecutionState,
    source_token: TestReference,
    relationship_name: RelationshipName,
    expected_target_tokens: Vec<TestReference>,
    expected_error: Option<HolonErrorKind>,
) {
    let context = state
        .open_assertion_context("assert_related_holons")
        .await
        .expect("failed to open assertion transaction for assert_related_holons");

    let assertion_result: Result<(), HolonError> = (|| {
        let source_reference =
            state.resolve_execution_reference(&context, ResolveBy::Source, &source_token)?;
        let expected_target_references = state.resolve_execution_references(
            &context,
            ResolveBy::Expected,
            &expected_target_tokens,
        )?;

        let related_holons_handle = source_reference.related_holons(&relationship_name)?;
        let related_holons = related_holons_handle.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on related holons collection for {}: {}",
                relationship_name.0.0, e
            ))
        })?;

        let mut actual_identities = related_holons
            .get_members()
            .iter()
            .map(reference_identity)
            .collect::<Result<Vec<_>, _>>()?;
        actual_identities.sort();

        let mut expected_identities = expected_target_references
            .iter()
            .map(reference_identity)
            .collect::<Result<Vec<_>, _>>()?;
        expected_identities.sort();

        assert_eq!(
            expected_identities,
            actual_identities,
            "assert_related_holons mismatch for relationship '{}'",
            relationship_name.0.0
        );

        Ok(())
    })();

    match assertion_result {
        Ok(()) => {
            assert!(
                expected_error.is_none(),
                "assert_related_holons succeeded but expected {:?}",
                expected_error,
            );
            info!(
                "Success! Related holons matched expected for '{}'",
                relationship_name.0.0
            );
        }
        Err(error) => {
            let actual = HolonErrorKind::from(&error);
            assert_eq!(
                Some(actual),
                expected_error,
                "assert_related_holons: unexpected error {:?}",
                error,
            );
        }
    }
}
