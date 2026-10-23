use std::sync::Arc;

use core_types::{HolonError, HolonId, TemporaryId};

use crate::core_shared_objects::holon::state::StagedState;
use crate::core_shared_objects::transactions::TransactionContext;
use crate::reference_layer::assert_reference_transaction_compatible;
use crate::{HolonReference, SmartReference};

/// Logical holon identity across versions and branches, independently of keys.
/// New lineages are transaction-local until publication establishes their saved root.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LineageIdentity {
    /// Record-derived lineage root, retaining the space of external references.
    Saved(HolonId),
    /// A transaction-local unsaved lineage: transient input or a staged create/clone.
    New(TemporaryId),
}

impl std::fmt::Display for LineageIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Saved(id) => write!(formatter, "{id}"),
            Self::New(id) => write!(formatter, "staged {}", id.0),
        }
    }
}

impl Ord for LineageIdentity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, other) {
            (Self::Saved(a), Self::Saved(b)) => a.to_canonical_bytes().cmp(&b.to_canonical_bytes()),
            (Self::New(a), Self::New(b)) => a.cmp(b),
            (Self::Saved(_), Self::New(_)) => Ordering::Less,
            (Self::New(_), Self::Saved(_)) => Ordering::Greater,
        }
    }
}
impl PartialOrd for LineageIdentity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl LineageIdentity {
    /// Reads saved lineage metadata through bound references; no key lookup or head
    /// selection participates. Mutable references must belong to the active transaction.
    pub fn for_reference(
        reference: &HolonReference,
        context: &Arc<TransactionContext>,
    ) -> Result<Self, HolonError> {
        assert_reference_transaction_compatible(reference, context)?;
        match reference {
            HolonReference::Smart(saved) => Ok(Self::Saved(saved.lineage_root_id()?)),
            HolonReference::Staged(staged) => {
                let (state, source) = staged.staged_state_and_source()?;
                let saved_id = match state {
                    StagedState::ForCreate => return Ok(Self::New(staged.temporary_id())),
                    StagedState::ForUpdate
                    | StagedState::ForUpdateGraphOnly
                    | StagedState::ForUpdateNewVersion => source.ok_or_else(|| {
                        HolonError::InvalidState(format!(
                            "Staged update {:?} has no saved source",
                            staged.temporary_id()
                        ))
                    })?,
                    StagedState::Committed(id) => id,
                    StagedState::Abandoned => {
                        return Err(HolonError::InvalidState(format!(
                            "Abandoned candidate {:?} has no assessable lineage",
                            staged.temporary_id()
                        )))
                    }
                };
                let saved = SmartReference::new_from_id(
                    context.space_read_handle(),
                    HolonId::Local(saved_id),
                );
                Ok(Self::Saved(saved.lineage_root_id()?))
            }
            HolonReference::Transient(transient) => Ok(Self::New(transient.temporary_id())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::{holon::SavedHolon, Holon};
    use crate::descriptors::test_support::{
        build_context, build_context_with_saved_holons, new_test_holon, stage_update_snapshot,
    };
    use base_types::MapInteger;
    use core_types::{LocalId, PropertyMap};
    use std::collections::HashMap;

    #[test]
    fn graph_only_and_committed_handles_use_record_lineage_metadata() -> Result<(), HolonError> {
        let source = LocalId(vec![1; 39]);
        let published = LocalId(vec![2; 39]);
        let root = LocalId(vec![3; 39]);
        let context = build_context_with_saved_holons(
            [source.clone(), published.clone()]
                .into_iter()
                .map(|id| {
                    SavedHolon::new(id, PropertyMap::new(), Some(root.clone()), MapInteger(2))
                })
                .collect(),
            HashMap::new(),
        );
        let staged = stage_update_snapshot(&context, source, new_test_holon(&context, "update")?)?;
        {
            let rc = staged.get_holon_to_commit(&context)?;
            let mut holon = rc.write().unwrap();
            let Holon::Staged(holon) = &mut *holon else { panic!("staged update") };
            // This clone carries no lineage metadata. The saved record is authoritative.
            assert!(holon.original_id_ref().is_none());
            holon.note_relationship_mutation(false)?;
        }
        let expected = LineageIdentity::Saved(HolonId::Local(root));
        assert_eq!(LineageIdentity::for_reference(&staged.clone().into(), &context)?, expected);
        {
            let rc = staged.get_holon_to_commit(&context)?;
            let mut holon = rc.write().unwrap();
            let Holon::Staged(holon) = &mut *holon else { panic!("staged update") };
            holon.to_committed(published)?;
        }
        assert_eq!(LineageIdentity::for_reference(&staged.into(), &context)?, expected);
        Ok(())
    }

    #[test]
    fn lineage_resolution_preserves_operational_and_transaction_errors() -> Result<(), HolonError> {
        let context = build_context();
        let other = build_context();
        let missing = HolonReference::smart_from_id(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![1; 39])),
        );
        assert!(matches!(
            LineageIdentity::for_reference(&missing, &context),
            Err(HolonError::HolonNotFound(_))
        ));
        for reference in [
            new_test_holon(&other, "transient")?.into(),
            other.mutation().stage_new_holon(new_test_holon(&other, "staged")?)?.into(),
        ] {
            assert!(matches!(
                LineageIdentity::for_reference(&reference, &context),
                Err(HolonError::CrossTransactionReference { .. })
            ));
        }
        Ok(())
    }
}
