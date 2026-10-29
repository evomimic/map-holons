//! Transaction-scoped lookup of Core descriptor identities.

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
        Err(HolonError::HolonNotFound(_)) => {
            Ok(context.lookup().get_saved_holon_by_key(&key)?.into())
        }
        Err(error) => Err(error),
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
