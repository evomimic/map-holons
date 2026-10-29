//! Core role recognition on encountered definitions, without keyed discovery.
use super::walk_extends_chain;
use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::MapString;
use core_types::HolonError;
use type_names::CoreDefinitionRole;

/// Reads the definition's record key rather than a cached relationship occurrence key.
pub(super) fn record_key(subject: &HolonReference) -> Result<Option<MapString>, HolonError> {
    match subject {
        HolonReference::Smart(reference) => reference.record_key(),
        _ => subject.key(),
    }
}

/// Tests the subject's own role, without classifying its ancestors.
pub fn has_role(subject: &HolonReference, role: CoreDefinitionRole) -> Result<bool, HolonError> {
    Ok(record_key(subject)?.is_some_and(|key| key.0 == role.key()))
}

/// Recognizes a role in self-first order, with the kernel walk's short-circuit error semantics.
/// This selects no definition version and leaves ordinary identity comparisons unchanged.
pub fn extends_role(
    subject: &HolonReference,
    role: CoreDefinitionRole,
) -> Result<bool, HolonError> {
    for ancestor in walk_extends_chain(subject) {
        if has_role(&ancestor?, role)? {
            return Ok(true);
        }
    }
    Ok(false)
}
