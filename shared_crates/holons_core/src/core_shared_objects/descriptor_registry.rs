//! Session-owned prepared schema and explicit transaction-local snapshots.
use super::{space_manager::HolonSpaceManager, transactions::TransactionContext};
use crate::{
    descriptors::{resolve_core_descriptor, EffectiveHolonDescriptor},
    HolonReference, ReadableHolon, TransientReference, WritableHolon,
};
use core_types::{HolonError, HolonId, TemporaryId};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use type_names::{CoreHolonTypeName, CoreRelationshipTypeName};

#[derive(Default)]
struct RegistryState {
    context: Option<Arc<TransactionContext>>,
    root: Option<TemporaryId>,
    entries: HashMap<HolonId, TemporaryId>,
    admitted: std::collections::HashSet<TemporaryId>,
}

/// The agent Session owns this capability. The space keeps only a weak association.
/// Only saved dependencies may enter it; candidate schemas remain transaction-local.
pub struct EffectiveHolonDescriptorRegistry {
    space: Weak<HolonSpaceManager>,
    state: Mutex<RegistryState>,
}
impl EffectiveHolonDescriptorRegistry {
    pub fn new(space: &Arc<HolonSpaceManager>) -> Self {
        Self { space: Arc::downgrade(space), state: Default::default() }
    }

    /// A published entry establishes that Descriptor Runtime bootstrap is available.
    pub fn is_empty(&self) -> Result<bool, HolonError> {
        Ok(self.state.lock().map_err(lock_error)?.entries.is_empty())
    }

    pub fn context(&self) -> Result<Arc<TransactionContext>, HolonError> {
        let mut state = self.state.lock().map_err(lock_error)?;
        if let Some(context) = &state.context {
            return Ok(context.clone());
        }
        let space = self
            .space
            .upgrade()
            .ok_or_else(|| HolonError::ServiceNotAvailable("Session space".into()))?;
        let context =
            space.get_transaction_manager().open_session_derived_transaction(space.clone())?;
        state.context = Some(context.clone());
        Ok(context)
    }

    /// Read ingress may address this retained carrier without making it a public transaction.
    pub fn owned_context(
        &self,
        tx_id: super::transactions::TxId,
    ) -> Option<Arc<TransactionContext>> {
        let state = self.state.lock().ok()?;
        state.context.as_ref().filter(|context| context.tx_id() == tx_id).cloned()
    }

    /// Resolves only published descriptor handles; never exposes carrier envelopes or the registry root.
    pub fn reference_owner(
        &self,
        tx_id: super::transactions::TxId,
        id: &TemporaryId,
    ) -> Option<Arc<TransactionContext>> {
        let state = self.state.lock().ok()?;
        let context = state.context.as_ref()?;
        (context.tx_id() == tx_id && state.admitted.contains(id)).then(|| context.clone())
    }

    pub fn get(&self, target: &HolonId) -> Result<Option<EffectiveHolonDescriptor>, HolonError> {
        let state = self.state.lock().map_err(lock_error)?;
        Ok(state.entries.get(target).map(|id| {
            EffectiveHolonDescriptor::from_constructed_holon(
                TransientReference::from_temporary_id(
                    state.context.as_ref().expect("entry owner").context_handle(),
                    id,
                )
                .into(),
            )
        }))
    }

    /// Checks reference phases only; admission performs no graph traversal or validation.
    pub fn ensure_saved_dependencies(
        descriptor: &EffectiveHolonDescriptor,
    ) -> Result<(), HolonError> {
        for (_, collection) in descriptor.holon().all_related_holons()?.iter() {
            let members = collection.read().map_err(lock_error)?.get_members().clone();
            if members.iter().any(|reference| !matches!(reference, HolonReference::Smart(_))) {
                return Err(HolonError::InvalidParameter(
                    "Session descriptors require saved dependencies".into(),
                ));
            }
        }
        Ok(())
    }

    /// Publishes a complete sealed result. All graph reads precede the publication lock.
    pub fn publish(
        &self,
        target: HolonId,
        descriptor: &EffectiveHolonDescriptor,
    ) -> Result<(), HolonError> {
        Self::ensure_saved_dependencies(descriptor)?;
        let context = self.context()?;
        let HolonReference::Transient(reference) = descriptor.holon() else {
            return Err(HolonError::InvalidParameter(
                "Prepared descriptor must be transient".into(),
            ));
        };
        if reference.tx_id() != context.tx_id() {
            return Err(HolonError::InvalidParameter(
                "Prepared descriptor has a different owner".into(),
            ));
        }
        let needs_root = self.state.lock().map_err(lock_error)?.root.is_none();
        let new_root = if needs_root {
            let descriptor_type = resolve_core_descriptor(
                &context,
                &CoreHolonTypeName::EffectiveHolonDescriptorRegistry
                    .descriptor_key(&CoreHolonTypeName::HolonType)
                    .0,
            )?;
            let mut root = context.mutation().new_holon(None)?;
            root.with_descriptor(descriptor_type)?;
            if let Some(owner) = context.get_space_holon()? {
                root.add_related_holons(CoreRelationshipTypeName::OwnedBy, vec![owner])?;
            }
            Some(root)
        } else {
            None
        };
        let mut state = self.state.lock().map_err(lock_error)?;
        if state.root.is_none() {
            state.root = new_root.map(|root| root.temporary_id());
        }
        let mut root = TransientReference::from_temporary_id(
            context.context_handle(),
            state.root.as_ref().expect("registry root"),
        );
        if let Some(previous) = state.entries.get(&target) {
            let previous =
                TransientReference::from_temporary_id(context.context_handle(), previous);
            root.remove_related_holons(
                CoreRelationshipTypeName::PreparedDescriptors,
                vec![previous.into()],
            )?;
        }
        root.add_related_holons(
            CoreRelationshipTypeName::PreparedDescriptors,
            vec![descriptor.holon().clone()],
        )?;
        state.admitted.insert(reference.temporary_id());
        state.entries.insert(target, reference.temporary_id());
        drop(state);
        self.prune_envelopes()
    }

    /// Removes construction envelopes, including after failed preparation attempts.
    pub fn prune_envelopes(&self) -> Result<(), HolonError> {
        let (context, retained) = {
            let state = self.state.lock().map_err(lock_error)?;
            let Some(context) = state.context.clone() else {
                return Ok(());
            };
            (
                context,
                state
                    .admitted
                    .iter()
                    .cloned()
                    .chain(state.root.iter().cloned())
                    .collect::<std::collections::HashSet<_>>(),
            )
        };
        let pool = context.export_transient_holons()?;
        let holons = pool
            .holons_by_id()
            .iter()
            .filter(|(id, _)| retained.contains(*id))
            .map(|(id, holon)| (id.clone(), holon.clone()))
            .collect();
        context.import_transient_holons(super::HolonPool::from_parts(holons, Default::default()))
    }

    /// Shallow-clones the registry and its members. Only registry entries are rebound;
    /// each descriptor's saved schema references retain their original identities.
    pub fn snapshot_into(
        &self,
        destination: &Arc<TransactionContext>,
    ) -> Result<Option<TransientReference>, HolonError> {
        let source = {
            let state = self.state.lock().map_err(lock_error)?;
            state.root.as_ref().map(|id| {
                HolonReference::from(TransientReference::from_temporary_id(
                    state.context.as_ref().expect("registry owner").context_handle(),
                    id,
                ))
            })
        };
        let Some(source) = source else {
            return Ok(None);
        };
        // Freeze membership before cloning entries. Clone default population may
        // prepare and publish additional Session descriptors reentrantly; those
        // later entries belong to a subsequent snapshot, never this copied root.
        let mut root = destination.clone_holon(&source)?;
        let members = root
            .related_holons(CoreRelationshipTypeName::PreparedDescriptors)?
            .read()
            .map_err(lock_error)?
            .get_members()
            .clone();
        let mut copies = Vec::with_capacity(members.len());
        for member in &members {
            let descriptor = EffectiveHolonDescriptor::from_constructed_holon(member.clone());
            Self::ensure_saved_dependencies(&descriptor)?;
            let copy = destination.clone_holon(member)?;
            copy.mark_as_immutable()?;
            copies.push(copy.into());
        }
        root.remove_related_holons(CoreRelationshipTypeName::PreparedDescriptors, members)?;
        root.add_related_holons(CoreRelationshipTypeName::PreparedDescriptors, copies)?;
        root.mark_as_immutable()?;
        destination.set_descriptor_registry_root(&root)?;
        Ok(Some(root))
    }
}
impl Drop for EffectiveHolonDescriptorRegistry {
    fn drop(&mut self) {
        if let Ok(state) = self.state.get_mut() {
            if let Some(context) = &state.context {
                let _ = context.dispose();
            }
        }
    }
}
fn lock_error(error: impl std::fmt::Display) -> HolonError {
    HolonError::FailedToAcquireLock(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::build_context_with_saved_holons;
    use core_types::LocalId;

    fn fixture() -> Result<
        (
            Arc<EffectiveHolonDescriptorRegistry>,
            Arc<TransactionContext>,
            EffectiveHolonDescriptor,
            HolonId,
        ),
        HolonError,
    > {
        let saved = crate::core_shared_objects::SavedHolon::new(
            LocalId::from_bytes(vec![7; 39]),
            Default::default(),
            None,
            base_types::MapInteger(1),
        );
        let caller = build_context_with_saved_holons(vec![saved], Default::default());
        let space = caller.space_manager();
        let registry = Arc::new(EffectiveHolonDescriptorRegistry::new(&space));
        space.attach_descriptor_registry(&registry);
        let owner = registry.context()?;
        let root = owner.mutation().new_holon(None)?;
        registry.state.lock().unwrap().root = Some(root.temporary_id());
        let target = HolonId::Local(LocalId::from_bytes(vec![7; 39]));
        let saved = HolonReference::smart_from_id(owner.space_read_handle(), target.clone());
        let mut descriptor = owner.mutation().new_holon(None)?;
        descriptor.add_related_holons(
            CoreRelationshipTypeName::DescribedHolonType,
            vec![saved.clone()],
        )?;
        descriptor
            .add_related_holons(CoreRelationshipTypeName::EffectiveConstraints, vec![saved])?;
        descriptor.mark_as_immutable()?;
        Ok((
            registry,
            caller,
            EffectiveHolonDescriptor::from_constructed_holon(descriptor.into()),
            target,
        ))
    }

    #[test]
    fn snapshot_copies_only_registry_and_results_and_preserves_saved_targets(
    ) -> Result<(), HolonError> {
        let (registry, destination, descriptor, target) = fixture()?;
        registry.publish(target.clone(), &descriptor)?;
        let root = registry.snapshot_into(&destination)?.unwrap();
        let members = root
            .related_holons(CoreRelationshipTypeName::PreparedDescriptors)?
            .read()
            .unwrap()
            .get_members()
            .clone();
        assert_eq!(members.len(), 1);
        let HolonReference::Transient(copy) = &members[0] else {
            panic!("expected transient copy")
        };
        assert_eq!(copy.tx_id(), destination.tx_id());
        assert_eq!(destination.lookup().transient_count()?, 2);
        let prepared = EffectiveHolonDescriptor::find_in_context(
            &destination,
            &HolonReference::smart_from_id(destination.space_read_handle(), target.clone()),
        )?
        .unwrap();
        for (_, collection) in prepared.holon().all_related_holons()?.iter() {
            for member in collection.read().unwrap().get_members() {
                assert!(matches!(member, HolonReference::Smart(_)));
                assert_eq!(member.holon_id()?, target);
            }
        }
        destination.dispose()?;
        assert!(descriptor.holon().key()?.is_none());
        drop(registry);
        assert!(descriptor.holon().key().is_err());
        Ok(())
    }

    #[test]
    fn foreign_unsaved_dependencies_are_rejected_before_publication_or_snapshot(
    ) -> Result<(), HolonError> {
        let (registry, foreign, _, target) = fixture()?;
        let dependency: HolonReference = foreign.mutation().new_holon(None)?.into();
        let owner = registry.context()?;
        let mut result = owner.mutation().new_holon(None)?;
        result
            .add_related_holons(CoreRelationshipTypeName::EffectiveConstraints, vec![dependency])?;
        result.mark_as_immutable()?;
        let result = EffectiveHolonDescriptor::from_constructed_holon(result.into());
        assert!(registry.publish(target.clone(), &result).is_err());
        assert!(registry.get(&target)?.is_none());
        let staged: HolonReference = foreign
            .mutation()
            .stage_new_holon(
                foreign.mutation().new_holon(Some("foreign-staged-dependency".into()))?,
            )?
            .into();
        let mut other = owner.mutation().new_holon(None)?;
        other.add_related_holons(CoreRelationshipTypeName::EffectiveConstraints, vec![staged])?;
        assert!(EffectiveHolonDescriptorRegistry::ensure_saved_dependencies(
            &EffectiveHolonDescriptor::from_constructed_holon(other.into())
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn hits_allocate_nothing_and_old_handles_survive_selected_view_replacement(
    ) -> Result<(), HolonError> {
        let (registry, _, first, target) = fixture()?;
        registry.publish(target.clone(), &first)?;
        let owner = registry.context()?;
        let before = owner.lookup().transient_count()?;
        for _ in 0..8 {
            assert!(registry.get(&target)?.is_some());
        }
        assert_eq!(owner.lookup().transient_count()?, before);
        let replacement = owner.clone_holon(first.holon())?;
        replacement.mark_as_immutable()?;
        registry.publish(
            target.clone(),
            &EffectiveHolonDescriptor::from_constructed_holon(replacement.into()),
        )?;
        assert_ne!(
            registry.get(&target)?.unwrap().holon().full_reference_id_string(),
            first.holon().full_reference_id_string()
        );
        assert!(first.holon().key()?.is_none());
        let HolonReference::Transient(first_reference) = first.holon() else { unreachable!() };
        assert!(registry
            .reference_owner(first_reference.tx_id(), &first_reference.temporary_id())
            .is_some());
        Ok(())
    }
}
