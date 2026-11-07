//! Canonical Core descriptor lookup in saved or transaction-local scopes.

use crate::core_shared_objects::transactions::TransactionContext;
use crate::reference_layer::HolonReference;
use base_types::MapString;
use core_types::HolonError;
use std::sync::Arc;

/// Resolves a canonical Core schema key through the transaction lookup façade.
///
/// Staged definitions take precedence; only `HolonNotFound` permits saved lookup.
/// Duplicate-key and operational failures propagate unchanged. This resolves schema
/// identities (including rule holons), without asserting a particular descriptor kind.
/// Resolve at the pass boundary and compare the resulting bound references by identity;
/// do not reuse them across transactions or schema mutations.
pub fn resolve_core_descriptor(
    context: &Arc<TransactionContext>,
    key: &str,
) -> Result<HolonReference, HolonError> {
    let key = MapString(key.to_owned());
    match context.lookup().get_staged_holon_by_base_key(&key) {
        Ok(reference) => Ok(reference.into()),
        Err(HolonError::HolonNotFound(_)) => resolve_saved_core_descriptor(context, key),
        Err(error) => Err(error),
    }
}

/// Saved state changes within a transaction only through its own staged candidates, which
/// take precedence above, so one saved key lookup serves the rest of the transaction.
/// Failures are not retained.
fn resolve_saved_core_descriptor(
    context: &Arc<TransactionContext>,
    key: MapString,
) -> Result<HolonReference, HolonError> {
    let lock_error = |error: std::sync::PoisonError<_>| {
        HolonError::FailedToAcquireLock(format!("Saved Core descriptor lookup: {error}"))
    };
    if let Some(reference) = context.saved_core_descriptors.lock().map_err(lock_error)?.get(&key.0)
    {
        return Ok(reference.clone());
    }
    let reference: HolonReference = context.lookup().get_saved_holon_by_key(&key)?.into();
    context.saved_core_descriptors.lock().map_err(lock_error)?.insert(key.0, reference.clone());
    Ok(reference)
}

/// Resolves a canonical identity in the source's real scope. Transaction-local
/// references keep staged-first semantics; saved references resolve saved state
/// through their Space, using a bounded context only for the service lookup.
pub(crate) fn resolve_core_descriptor_for(
    source: &HolonReference,
    key: &str,
) -> Result<HolonReference, HolonError> {
    if let Some(context) = source.transaction_context() {
        resolve_core_descriptor(&context, key)
    } else {
        Ok(source.space_read_handle().get_saved_holon_by_key(&MapString(key.into()))?.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_test_holon};

    #[test]
    fn staged_core_definition_precedes_saved_lookup() -> Result<(), HolonError> {
        let context = build_context();
        let staged = context
            .mutation()
            .stage_new_holon(new_test_holon(&context, "StringValueType.ValueType")?)?;
        assert_eq!(resolve_core_descriptor(&context, "StringValueType.ValueType")?, staged.into());
        Ok(())
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    use crate::core_shared_objects::holon::SavedHolon;
    use crate::descriptors::test_support::{build_context_with_saved_holons, new_test_holon};
    use crate::ReadableHolon;
    use base_types::{BaseValue, MapInteger};
    use core_types::{HolonId, LocalId, PropertyMap, PropertyName};

    #[test]
    fn saved_scope_ignores_staged_overrides_but_transaction_scope_preserves_them(
    ) -> Result<(), HolonError> {
        let key = "StringValueType.ValueType";
        let id = HolonId::Local(LocalId(vec![37; 39]));
        let saved = SavedHolon::new(
            id.local_id().clone(),
            PropertyMap::from([(PropertyName("Key".into()), BaseValue::StringValue(key.into()))]),
            None,
            MapInteger(1),
        );
        let context = build_context_with_saved_holons(vec![saved], Default::default());
        let staged: HolonReference =
            context.mutation().stage_new_holon(new_test_holon(&context, key)?)?.into();
        let source: HolonReference =
            crate::SmartReference::new_from_id(context.space_read_handle(), id.clone()).into();
        assert_eq!(resolve_core_descriptor_for(&source, key)?.holon_id()?, id);
        assert_eq!(resolve_core_descriptor_for(&staged, key)?, staged);
        assert_eq!(resolve_core_descriptor(&context, key)?, staged);
        assert!(context.is_open());
        Ok(())
    }

    #[test]
    fn saved_resolution_is_reused_without_shadowing_later_staged_definitions(
    ) -> Result<(), HolonError> {
        let key = "StringValueType.ValueType";
        let id = HolonId::Local(LocalId(vec![38; 39]));
        let saved = SavedHolon::new(
            id.local_id().clone(),
            PropertyMap::from([(PropertyName("Key".into()), BaseValue::StringValue(key.into()))]),
            None,
            MapInteger(1),
        );
        let context = build_context_with_saved_holons(vec![saved], Default::default());
        assert!(resolve_core_descriptor(&context, "Missing.HolonType").is_err());
        assert_eq!(resolve_core_descriptor(&context, key)?.holon_id()?, id);
        assert_eq!(resolve_core_descriptor(&context, key)?.holon_id()?, id);
        assert_eq!(
            context.saved_core_descriptors.lock().unwrap().keys().collect::<Vec<_>>(),
            [key]
        );
        let staged: HolonReference =
            context.mutation().stage_new_holon(new_test_holon(&context, key)?)?.into();
        assert_eq!(resolve_core_descriptor(&context, key)?, staged);
        Ok(())
    }

    #[test]
    fn saved_scope_preserves_missing_and_duplicate_key_errors() -> Result<(), HolonError> {
        let key = "Duplicate.HolonType";
        let saved = |value| {
            SavedHolon::new(
                LocalId(vec![value; 39]),
                PropertyMap::from([(
                    PropertyName("Key".into()),
                    BaseValue::StringValue(key.into()),
                )]),
                None,
                MapInteger(1),
            )
        };
        let context = build_context_with_saved_holons(vec![saved(1), saved(2)], Default::default());
        let source = HolonReference::smart_from_id(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![1; 39])),
        );
        assert!(matches!(
            resolve_core_descriptor_for(&source, key),
            Err(HolonError::DuplicateError(..))
        ));
        assert!(matches!(
            resolve_core_descriptor_for(&source, "Missing.HolonType"),
            Err(HolonError::HolonNotFound(_))
        ));
        Ok(())
    }
}
