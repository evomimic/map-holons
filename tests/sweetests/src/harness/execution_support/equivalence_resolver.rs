//! Sweettest reference translation for definitional-equivalence assertions.
//!
//! Fixture snapshots can refer to holons that have since been committed, and
//! saved-lookup stubs intentionally stand in for pre-existing saved content.
//! This resolver adapts those harness concepts to the phase-generic resolver
//! port exposed by `holons_core`.

use crate::{ExecutionHolons, FixtureHeadIndex, SAVED_LOOKUP_STUB_MARKER};
use holons_prelude::prelude::*;
use type_names::property_names::ToPropertyName;

/// Resolver used by saved-content assertions.
pub struct ExecutionEquivalenceResolver<'a> {
    execution_holons: &'a ExecutionHolons,
    fixture_head_index: &'a FixtureHeadIndex,
}

impl<'a> ExecutionEquivalenceResolver<'a> {
    /// Creates a resolver over the execution registry for the current test run.
    pub fn new(
        execution_holons: &'a ExecutionHolons,
        fixture_head_index: &'a FixtureHeadIndex,
    ) -> Self {
        Self { execution_holons, fixture_head_index }
    }
}

impl EquivalenceResolver for ExecutionEquivalenceResolver<'_> {
    fn resolve(&self, reference: &HolonReference) -> Result<NodeResolution, HolonError> {
        if is_saved_lookup_stub(reference)? {
            return Ok(NodeResolution::MatchByKey);
        }

        if let Some(resolved_reference) =
            canonical_saved_reference(reference, self.execution_holons, self.fixture_head_index)?
        {
            return Ok(NodeResolution::Canonical(resolved_reference));
        }

        Ok(NodeResolution::AsIs)
    }
}

/// Detects fixture saved-lookup stubs without forcing a saved-reference fetch.
fn is_saved_lookup_stub(reference: &HolonReference) -> Result<bool, HolonError> {
    match reference {
        HolonReference::Smart(smart) => {
            // A saved-lookup stub may be a SmartReference with only cached
            // marker/key properties. Calling `property_value()` would try to
            // fetch backing saved content, which stubs intentionally lack.
            Ok(smart.smart_property_values().is_some_and(|properties| {
                properties.contains_key(&SAVED_LOOKUP_STUB_MARKER.to_property_name())
            }))
        }
        HolonReference::Transient(_) | HolonReference::Staged(_) => {
            Ok(reference.property_value(SAVED_LOOKUP_STUB_MARKER)?.is_some())
        }
    }
}

/// Maps fixture transient/staged snapshots to the saved reference recorded after commit.
fn canonical_saved_reference(
    reference: &HolonReference,
    execution_holons: &ExecutionHolons,
    fixture_head_index: &FixtureHeadIndex,
) -> Result<Option<HolonReference>, HolonError> {
    let snapshot_id = match reference {
        HolonReference::Transient(transient) => transient.temporary_id(),
        HolonReference::Staged(staged) => staged.temporary_id(),
        HolonReference::Smart(_) => return Ok(None),
    };

    let head_id = fixture_head_index.get(&snapshot_id).unwrap_or(&snapshot_id);

    let Some(resolved) = execution_holons.by_snapshot_id.get(head_id) else {
        return Ok(None);
    };

    let resolved_reference = resolved.execution_handle.get_holon_reference()?;
    if resolved_reference.is_saved() {
        Ok(Some(resolved_reference))
    } else {
        Ok(None)
    }
}
