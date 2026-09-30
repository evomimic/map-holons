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

/// Resolves a canonical key for prospective assessment, distinguishing competing updates
/// from unrelated same-key definitions. Selection never chooses one competitor's content.
pub fn resolve_core_descriptor_with_reader<R: super::DescriptorReader>(
    context: &Arc<TransactionContext>,
    key: &str,
    reader: &R,
) -> Result<HolonReference, R::Error> {
    let key = MapString(key.to_owned());
    let staged = match context.lookup().get_staged_holons_by_base_key(&key) {
        Ok(staged) => staged,
        Err(HolonError::HolonNotFound(_)) => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    let mut live = Vec::new();
    for candidate in staged {
        if candidate.is_live_validation_candidate()? {
            live.push(HolonReference::from(candidate));
        }
    }
    let Some(first) = live.first() else {
        return reader.select(&context.lookup().get_saved_holon_by_key(&key)?.into());
    };
    let identity = crate::ProspectiveIdentity::for_reference(first, context)?;
    for other in &live[1..] {
        if crate::ProspectiveIdentity::for_reference(other, context)? != identity {
            // Unrelated creates do not acquire canonical identity by sharing a key.
            // Anchor resolution cannot choose which is Core; preserve the existing
            // ambiguous-key operational error until the owning key invariant diagnoses it.
            return Err(
                HolonError::DuplicateError("Core descriptor key".into(), key.to_string()).into()
            );
        }
    }
    if live.len() > 1 {
        return match identity {
            crate::ProspectiveIdentity::Saved(source) => {
                // A key lookup names the saved definition, not either staged handle.
                // Ask the reader to select its successor so competition remains visible.
                let saved = HolonReference::smart_from_id(context.space_read_handle(), source);
                reader.select(&saved)
            }
            _ => {
                Err(HolonError::DuplicateError("Core descriptor key".into(), key.to_string())
                    .into())
            }
        };
    }
    reader.select(first)
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
