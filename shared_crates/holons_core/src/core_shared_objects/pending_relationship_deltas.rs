//! Mutation-time summaries for supported graph-only overlap assessment.
use crate::HolonReference;
use core_types::{HolonId, TemporaryId};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BTreeSet};

/// Reference identity of a changed target, independent of its cached key or definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingRelationshipTarget {
    Saved(HolonId),
    Staged(TemporaryId),
}
impl Ord for PendingRelationshipTarget {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Saved(a), Self::Saved(b)) => a.to_canonical_bytes().cmp(&b.to_canonical_bytes()),
            (Self::Staged(a), Self::Staged(b)) => a.cmp(b),
            (Self::Saved(_), Self::Staged(_)) => Ordering::Less,
            (Self::Staged(_), Self::Saved(_)) => Ordering::Greater,
        }
    }
}
impl PartialOrd for PendingRelationshipTarget {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PendingRelationshipTarget {
    /// Captures identity without reading target content or saved collections.
    pub fn for_reference(reference: &HolonReference) -> Self {
        match reference {
            HolonReference::Smart(saved) => Self::Saved(saved.holon_id()),
            HolonReference::Staged(staged) => Self::Staged(staged.temporary_id()),
            HolonReference::Transient(transient) => Self::Staged(transient.temporary_id()),
        }
    }
}

/// Netted target changes for one staged relationship. With `repeated_occurrence`, this
/// is a conservative summary, not exact removal instructions: adding an absent target
/// twice and removing it can leave `removed` even though the collection is unchanged.
/// Consumers needing occurrence-level persistence must supply occurrence identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDelta {
    pub added: BTreeSet<PendingRelationshipTarget>,
    pub removed: BTreeSet<PendingRelationshipTarget>,
    pub repeated_occurrence: bool,
}
impl PendingDelta {
    pub(crate) fn record_add(&mut self, target: PendingRelationshipTarget, present: bool) {
        if present {
            self.repeated_occurrence = true;
            self.added.insert(target);
        } else if !self.removed.remove(&target) {
            self.added.insert(target);
        }
    }
    pub(crate) fn record_remove(&mut self, target: PendingRelationshipTarget) {
        if !self.added.remove(&target) || self.repeated_occurrence {
            self.removed.insert(target);
        }
    }
    /// Whether this relationship contributes an actual or conservatively retained change.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && !self.repeated_occurrence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core_shared_objects::{
            holon::{HolonCloneModel, StagedState},
            StagedHolon, WriteableHolonState,
        },
        descriptors::test_support::build_context,
        RelationshipMap,
    };
    use base_types::MapInteger;
    use core_types::{LocalId, PropertyMap, RelationshipName};

    #[test]
    fn removing_hydrated_duplicates_cannot_cancel_into_an_exact_empty_delta(
    ) -> Result<(), core_types::HolonError> {
        use crate::core_shared_objects::WritableRelationship;
        let context = build_context();
        let target: HolonReference = crate::SmartReference::new_with_properties(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![9])),
            PropertyMap::from([(
                core_types::PropertyName("Key".into()),
                base_types::BaseValue::StringValue("target".into()),
            )]),
        )
        .into();
        let id = PendingRelationshipTarget::for_reference(&target);
        let name = RelationshipName("R".into());
        let mut relationships = crate::StagedRelationshipMap::new_empty();
        relationships.add_related_holons_with_keys(
            name.clone(),
            vec![(target.clone(), None), (target.clone(), None)],
        )?;
        let mut staged = StagedHolon::new_for_update_from_clone_model(
            HolonCloneModel::new(
                MapInteger(1),
                None,
                PropertyMap::new(),
                Some(RelationshipMap::from(relationships)),
            ),
            LocalId(vec![1]),
        )?;
        assert!(staged.pending_relationship_deltas().is_empty(), "hydration records no mutation");
        staged.remove_related_holons_with_keys(&name, vec![(target.clone(), None)])?;
        let delta = &staged.pending_relationship_deltas()[&name];
        assert_eq!(delta.removed, BTreeSet::from([id]));
        assert!(delta.repeated_occurrence);
        staged.add_related_holons_with_keys(name.clone(), vec![(target, None)])?;
        assert_eq!(staged.get_staged_relationship(&name)?.read().unwrap().get_members().len(), 1);
        let delta = &staged.pending_relationship_deltas()[&name];
        assert!(
            delta.repeated_occurrence,
            "two occurrences became one: the summary remains conservative"
        );
        assert!(!delta.is_empty());
        Ok(())
    }

    #[test]
    fn mutation_summaries_net_only_actual_changes_and_preserve_duplicates(
    ) -> Result<(), core_types::HolonError> {
        let context = build_context();
        let target: HolonReference = crate::SmartReference::new_with_properties(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![9])),
            PropertyMap::from([(
                core_types::PropertyName("Key".into()),
                base_types::BaseValue::StringValue("target".into()),
            )]),
        )
        .into();
        let id = PendingRelationshipTarget::for_reference(&target);
        let name = RelationshipName("R".into());
        let new_update = || {
            StagedHolon::new_for_update_from_clone_model(
                HolonCloneModel::new(
                    MapInteger(1),
                    None,
                    PropertyMap::new(),
                    Some(RelationshipMap::new_empty()),
                ),
                LocalId(vec![1]),
            )
        };
        let mut staged = new_update()?;
        let add = |staged: &mut StagedHolon| {
            staged
                .add_related_holons_with_keys(name.clone(), vec![(target.clone(), None)])
                .map(|_| ())
        };
        let remove = |staged: &mut StagedHolon| {
            staged.remove_related_holons_with_keys(&name, vec![(target.clone(), None)]).map(|_| ())
        };
        add(&mut staged)?;
        remove(&mut staged)?;
        assert!(staged.pending_relationship_deltas().is_empty());
        remove(&mut staged)?;
        add(&mut staged)?;
        assert_eq!(staged.pending_relationship_deltas()[&name].added, BTreeSet::from([id.clone()]));
        assert!(staged.pending_relationship_deltas()[&name].removed.is_empty());
        assert!(!staged.pending_relationship_deltas()[&name].repeated_occurrence);
        // Repeated ungoverned additions retain their occurrence behavior.
        add(&mut staged)?;
        let collection = staged.get_staged_relationship(&name)?;
        assert_eq!(collection.read().unwrap().get_members().len(), 2);
        remove(&mut staged)?;
        assert!(collection.read().unwrap().get_members().is_empty());
        let delta = &staged.pending_relationship_deltas()[&name];
        assert!(delta.added.is_empty());
        assert_eq!(delta.removed, BTreeSet::from([id.clone()]));
        assert!(delta.repeated_occurrence);
        assert_eq!(staged.staged_state(), &StagedState::ForUpdate);

        // Hydration never records a delta; adding a preexisting target is still a repeat.
        let mut initial = crate::StagedRelationshipMap::new_empty();
        use crate::core_shared_objects::WritableRelationship;
        initial.add_related_holons_with_keys(name.clone(), vec![(target.clone(), None)])?;
        let mut staged = StagedHolon::new_for_update_from_clone_model(
            HolonCloneModel::new(
                MapInteger(1),
                None,
                PropertyMap::new(),
                Some(RelationshipMap::from(initial)),
            ),
            LocalId(vec![1]),
        )?;
        assert!(staged.pending_relationship_deltas().is_empty());
        add(&mut staged)?;
        remove(&mut staged)?;
        assert_eq!(staged.pending_relationship_deltas()[&name].removed, BTreeSet::from([id]));
        assert!(staged.pending_relationship_deltas()[&name].repeated_occurrence);
        let mut create = StagedHolon::new_for_create();
        add(&mut create)?;
        assert!(create.pending_relationship_deltas().is_empty());
        Ok(())
    }
}
