//! Descriptor lookup for transient loader responses, including schema bootstrap.
use holons_prelude::prelude::*;
use std::sync::Arc;

/// Only explicit bootstrap provisioning may return an untyped response when the
/// descriptor is absent. Ambiguity and operational failures remain errors.
pub(crate) fn resolve_response_descriptor(
    context: &Arc<TransactionContext>,
    key: &str,
) -> Result<Option<HolonReference>, HolonError> {
    let key = MapString::from(key);
    let matches = match context.lookup().get_staged_holons_by_base_key(&key) {
        Ok(matches) => matches,
        Err(HolonError::HolonNotFound(_)) => Vec::new(),
        Err(error) => return Err(error),
    };
    match matches.len() {
        1 => return Ok(Some(HolonReference::Staged(matches.into_iter().next().unwrap()))),
        0 => {}
        count => return Err(HolonError::DuplicateError(key.to_string(), count.to_string())),
    }
    // First-space provisioning has no saved namespace to query yet.
    if context.is_bootstrap_provisioning() && context.get_space_holon()?.is_none() {
        return Ok(None);
    }
    match context.lookup().get_saved_holon_by_key(&key) {
        Ok(reference) => Ok(Some(HolonReference::Smart(reference))),
        Err(HolonError::HolonNotFound(_)) if context.is_bootstrap_provisioning() => Ok(None),
        Err(error) => Err(error),
    }
}
