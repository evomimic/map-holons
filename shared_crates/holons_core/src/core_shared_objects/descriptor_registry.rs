//! Session-owned prepared results and Commit registry-owner capture/restoration.
use super::{space_manager::HolonSpaceManager, transactions::TransactionContext};
use crate::{
    descriptors::{
        resolve_core_descriptor, DerivedDescriptor, Descriptor, EffectiveHolonDescriptor,
    },
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
    derived_entries: HashMap<HolonId, (TemporaryId, Arc<super::space_session::SpaceSession>)>,
    admitted: std::collections::HashSet<TemporaryId>,
}

/// The runtime owner retains this participant; the space keeps only a weak association.
/// Space-managed cache Session resets retire its selected entries without requiring
/// reattachment. Only saved dependencies may enter it; candidates remain local.
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
        let state = self.state.lock().map_err(lock_error)?;
        Ok(state.entries.is_empty() && state.derived_entries.is_empty())
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

    pub fn get_derived(&self, target: &HolonId) -> Result<Option<DerivedDescriptor>, HolonError> {
        let state = self.state.lock().map_err(lock_error)?;
        Ok(state.derived_entries.get(target).map(|(id, session)| {
            DerivedDescriptor::from_constructed_holon(
                TransientReference::from_temporary_id(
                    state.context.as_ref().expect("entry owner").context_handle(),
                    id,
                )
                .into(),
            )
            .in_session(session.clone())
        }))
    }

    /// Checks reference phases only; admission performs no graph traversal or validation.
    pub fn ensure_saved_dependencies(
        descriptor: &EffectiveHolonDescriptor,
    ) -> Result<(), HolonError> {
        Self::ensure_saved_references(descriptor.holon())
    }

    fn ensure_saved_references(result: &HolonReference) -> Result<(), HolonError> {
        for (_, collection) in result.all_related_holons()?.iter() {
            let members = collection.read().map_err(lock_error)?.get_members().clone();
            if members.iter().any(|reference| !matches!(reference, HolonReference::Smart(_))) {
                return Err(HolonError::InvalidParameter(
                    "Session descriptors require saved dependencies".into(),
                ));
            }
        }
        Ok(())
    }

    /// Publishes a complete sealed HolonType projection.
    pub fn publish(
        &self,
        target: HolonId,
        descriptor: &EffectiveHolonDescriptor,
    ) -> Result<(), HolonError> {
        let session = self
            .space
            .upgrade()
            .ok_or_else(|| HolonError::ServiceNotAvailable("Session space".into()))?
            .session()?;
        self.publish_in_session(target, descriptor, &session)
    }

    /// Publishes a HolonType result only into the Session that initiated construction.
    /// This protects the shared carrier without migrating the HolonType wrapper.
    pub fn publish_in_session(
        &self,
        target: HolonId,
        descriptor: &EffectiveHolonDescriptor,
        session: &Arc<super::space_session::SpaceSession>,
    ) -> Result<(), HolonError> {
        self.publish_entry(target, descriptor.holon(), false, session)
    }

    /// Publishes a complete sealed value/property projection in its own partition.
    pub fn publish_derived(
        &self,
        target: HolonId,
        descriptor: &DerivedDescriptor,
        session: &Arc<super::space_session::SpaceSession>,
    ) -> Result<DerivedDescriptor, HolonError> {
        self.publish_entry(target.clone(), descriptor.holon(), true, session)?;
        self.get_derived(&target)?.ok_or(HolonError::ObsoleteSession)
    }

    /// Drops selected entries without disposing guest-received snapshots or the
    /// carrier used by an in-flight construction. Old HolonType handles retain
    /// their existing lifetime until that subsystem migrates to shared retirement.
    pub(super) fn reset_session(&self) -> Result<(), HolonError> {
        let mut state = self.state.lock().map_err(lock_error)?;
        let retired: Vec<_> = state.derived_entries.values().map(|(id, _)| id.clone()).collect();
        for id in retired {
            state.admitted.remove(&id);
        }
        state.derived_entries.clear();
        state.entries.clear();
        state.root = None;
        Ok(())
    }

    /// All graph reads precede the publication lock for either partition.
    fn publish_entry(
        &self,
        target: HolonId,
        descriptor: &HolonReference,
        derived: bool,
        session: &Arc<super::space_session::SpaceSession>,
    ) -> Result<(), HolonError> {
        session.check_active()?;
        Self::ensure_saved_references(descriptor)?;
        let context = self.context()?;
        let HolonReference::Transient(reference) = descriptor else {
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
        let publish = || {
            let mut state = self.state.lock().map_err(lock_error)?;
            if state.root.is_none() {
                state.root = new_root.map(|root| root.temporary_id());
            }
            let mut root = TransientReference::from_temporary_id(
                context.context_handle(),
                state.root.as_ref().expect("registry root"),
            );
            let previous = if derived {
                state.derived_entries.get(&target).map(|(id, _)| id)
            } else {
                state.entries.get(&target)
            };
            let relationship = if derived {
                CoreRelationshipTypeName::PreparedDerivedDescriptors
            } else {
                CoreRelationshipTypeName::PreparedDescriptors
            };
            if let Some(previous) = previous {
                let previous =
                    TransientReference::from_temporary_id(context.context_handle(), previous);
                root.remove_related_holons(relationship.clone(), vec![previous.into()])?;
            }
            root.add_related_holons(relationship, vec![descriptor.clone()])?;
            state.admitted.insert(reference.temporary_id());
            if derived {
                state.derived_entries.insert(target, (reference.temporary_id(), session.clone()));
            } else {
                state.entries.insert(target, reference.temporary_id());
            }
            Ok(())
        };
        session.publish(publish)?;
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

    /// Projects the retained owner's state without moving results into the agent pool.
    /// Retry if selected entries or the Session change while projection reads the pool.
    /// The projector is transport-only: it must not acquire descriptors or perform I/O.
    pub fn capture<T>(
        &self,
        mut project: impl FnMut(&Arc<TransactionContext>, &TemporaryId) -> Result<T, HolonError>,
    ) -> Result<Option<T>, HolonError> {
        self.capture_with_checkpoint(&mut project, &mut || {})
    }

    fn capture_with_checkpoint<T>(
        &self,
        project: &mut impl FnMut(&Arc<TransactionContext>, &TemporaryId) -> Result<T, HolonError>,
        checkpoint: &mut impl FnMut(),
    ) -> Result<Option<T>, HolonError> {
        loop {
            let space = self
                .space
                .upgrade()
                .ok_or_else(|| HolonError::ServiceNotAvailable("Session space".into()))?;
            let session = space.session()?;
            let (context, root, entries, derived) = {
                let state = self.state.lock().map_err(lock_error)?;
                (
                    state.context.clone(),
                    state.root.clone(),
                    state.entries.clone(),
                    state
                        .derived_entries
                        .iter()
                        .map(|(key, (id, _))| (key.clone(), id.clone()))
                        .collect::<HashMap<_, _>>(),
                )
            };
            let result = match (&context, &root) {
                (Some(context), Some(root)) => {
                    // Publication checks this too. Recheck the selected payloads at egress.
                    (|| {
                        for id in entries.values().chain(derived.values()) {
                            let result =
                                TransientReference::from_temporary_id(context.context_handle(), id);
                            Self::ensure_saved_references(&result.into())?;
                        }
                        project(context, root)
                    })()
                    .map(Some)
                }
                _ => Ok(None),
            };
            checkpoint();
            match session.check_active() {
                Err(HolonError::ObsoleteSession) => continue,
                Err(error) => return Err(error),
                Ok(()) => {}
            }
            let unchanged = {
                let state = self.state.lock().map_err(lock_error)?;
                state.root == root
                    && state.entries == entries
                    && state.derived_entries.len() == derived.len()
                    && state
                        .derived_entries
                        .iter()
                        .all(|(key, (id, _))| derived.get(key) == Some(id))
            };
            if unchanged {
                return result;
            }
        }
    }

    /// Restores a request-local registry owner and rebuilds its saved-ID indexes once.
    /// Binding supplies only the transported transient pool. No source reads, Extends
    /// traversal, or semantic construction is needed to index completed results.
    pub fn restore(
        agent: &Arc<TransactionContext>,
        owner_id: super::transactions::TxId,
        root_id: TemporaryId,
        bind: impl FnOnce(&Arc<TransactionContext>) -> Result<super::HolonPool, HolonError>,
    ) -> Result<Arc<Self>, HolonError> {
        if agent.descriptor_registry().is_some() {
            return Err(HolonError::InvalidParameter(
                "Space already has a descriptor registry".into(),
            ));
        }
        if owner_id == agent.tx_id() {
            return Err(HolonError::InvalidParameter(
                "Registry and agent transactions must be distinct".into(),
            ));
        }
        let space = agent.space_manager();
        let owner = space
            .get_transaction_manager()
            .open_private_restricted_cache_read_transaction_with_id(space.clone(), owner_id)?;
        // Own cleanup before binding: malformed input must not leave pool/context cycles.
        let registry = Arc::new(Self::new(&space));
        registry.state.lock().map_err(lock_error)?.context = Some(owner.clone());
        owner.import_transient_holons(bind(&owner)?)?;
        let root = TransientReference::from_temporary_id(owner.context_handle(), &root_id);
        let session = space.session()?;
        let mut entries = HashMap::new();
        let mut derived_entries = HashMap::new();
        let mut admitted = std::collections::HashSet::new();
        for (relationship, derived) in [
            (CoreRelationshipTypeName::PreparedDescriptors, false),
            (CoreRelationshipTypeName::PreparedDerivedDescriptors, true),
        ] {
            let members = root
                .related_holons(relationship)?
                .read()
                .map_err(lock_error)?
                .get_members()
                .clone();
            for member in members {
                let HolonReference::Transient(reference) = &member else {
                    return Err(HolonError::InvalidParameter(
                        "Registry entries must be transient results".into(),
                    ));
                };
                if reference.tx_id() != owner_id {
                    return Err(HolonError::InvalidParameter(
                        "Registry result has a different owner".into(),
                    ));
                }
                Self::ensure_saved_references(&member)?;
                let target = if derived {
                    DerivedDescriptor::from_constructed_holon(member.clone())
                        .lineage()?
                        .first()
                        .ok_or_else(|| {
                            HolonError::InvalidParameter("Missing derived source".into())
                        })?
                        .holon_id()?
                } else {
                    EffectiveHolonDescriptor::from_constructed_holon(member.clone())
                        .described_holon_type()?
                        .holon()
                        .holon_id()?
                };
                let duplicate = if derived {
                    derived_entries
                        .insert(target, (reference.temporary_id(), session.clone()))
                        .is_some()
                } else {
                    entries.insert(target, reference.temporary_id()).is_some()
                };
                if duplicate {
                    return Err(HolonError::InvalidParameter(
                        "Duplicate descriptor identity in registry snapshot".into(),
                    ));
                }
                admitted.insert(reference.temporary_id());
            }
        }
        {
            let mut state = registry.state.lock().map_err(lock_error)?;
            state.root = Some(root_id);
            state.entries = entries;
            state.derived_entries = derived_entries;
            state.admitted = admitted;
        }
        space.attach_descriptor_registry(&registry);
        Ok(registry)
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
    fn capture_preserves_registry_owner_and_does_not_populate_agent_pool() -> Result<(), HolonError>
    {
        let (registry, agent, descriptor, target) = fixture()?;
        registry.publish(target, &descriptor)?;
        let owner = registry.context()?;
        let before = agent.lookup().transient_count()?;
        let (owner_id, root_id, count) = registry
            .capture(|context, root| {
                Ok((context.tx_id(), root.clone(), context.lookup().transient_count()?))
            })?
            .unwrap();
        assert_eq!(owner_id, owner.tx_id());
        assert_ne!(owner_id, agent.tx_id());
        assert_eq!(Some(root_id), registry.state.lock().unwrap().root);
        assert_eq!(count, 2);
        assert_eq!(agent.lookup().transient_count()?, before);
        agent.dispose()?;
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

    #[test]
    fn saved_descriptor_hits_do_not_open_resolution_transactions() -> Result<(), HolonError> {
        let (registry, caller, descriptor, target) = fixture()?;
        registry.publish(target.clone(), &descriptor)?;
        let space = caller.space_manager();
        let before = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let saved = HolonReference::smart_from_id(caller.space_read_handle(), target);
        for _ in 0..8 {
            let prepared =
                EffectiveHolonDescriptor::find_prepared(&saved)?.expect("published descriptor");
            assert_eq!(prepared.holon(), descriptor.holon());
            assert_eq!(
                EffectiveHolonDescriptor::acquire(saved.clone())?.holon(),
                descriptor.holon()
            );
            assert!(EffectiveHolonDescriptor::find_for_instance(&saved)?.is_none());
        }
        let after = space.get_transaction_manager().open_public_transaction(space.clone())?;
        assert_eq!(after.tx_id().value(), before.tx_id().value() + 1);
        assert!(caller.is_open());
        Ok(())
    }
    #[test]
    fn derived_products_are_shared_by_identity_and_snapshot_without_another_lineage_walk(
    ) -> Result<(), HolonError> {
        use crate::descriptors::{
            inheritance::lineage_edge_reads, PropertyDescriptor, ValueDescriptor,
        };
        use base_types::{BaseValue, MapInteger, MapString};
        use core_types::PropertyMap;
        use type_names::{CoreRelationshipTypeName as R, ToPropertyName, ToRelationshipName};
        let ids =
            (31..=36).map(|n| HolonId::Local(core_types::LocalId(vec![n; 39]))).collect::<Vec<_>>();
        let saved = ids
            .iter()
            .enumerate()
            .map(|(index, id)| {
                super::super::SavedHolon::new(
                    id.local_id().clone(),
                    PropertyMap::from([
                        (
                            "Key".to_property_name(),
                            BaseValue::StringValue(MapString(format!("derived-test-{index}"))),
                        ),
                        (
                            "TypeName".to_property_name(),
                            BaseValue::StringValue(MapString(if index == 1 {
                                "StringValueType".into()
                            } else {
                                format!("derived-test-{index}")
                            })),
                        ),
                        (
                            "DefinesInstanceTypeKind".to_property_name(),
                            BaseValue::BooleanValue((index == 1).into()),
                        ),
                    ]),
                    None,
                    MapInteger(1),
                )
            })
            .collect();
        let context = build_context_with_saved_holons(
            saved,
            HashMap::from([
                ((ids[0].clone(), R::Extends.to_relationship_name()), vec![ids[1].clone()]),
                ((ids[1].clone(), R::Constraints.to_relationship_name()), vec![ids[2].clone()]),
                (
                    (ids[0].clone(), R::Constraints.to_relationship_name()),
                    vec![ids[3].clone(), ids[3].clone()],
                ),
                (
                    (ids[1].clone(), R::ValidationBindings.to_relationship_name()),
                    vec![ids[4].clone()],
                ),
                ((ids[1].clone(), R::AffordsOperator.to_relationship_name()), vec![ids[5].clone()]),
            ]),
        );
        let space = context.space_manager();
        let registry = Arc::new(EffectiveHolonDescriptorRegistry::new(&space));
        space.attach_descriptor_registry(&registry);
        let owner = registry.context()?;
        let root = owner.mutation().new_holon(None)?;
        registry.state.lock().unwrap().root = Some(root.temporary_id());
        let target = HolonReference::smart_from_id(context.space_read_handle(), ids[0].clone());
        let parent = HolonReference::smart_from_id(context.space_read_handle(), ids[1].clone());
        let first = ValueDescriptor::from_holon(target.clone());
        let before = lineage_edge_reads(&target);
        assert_eq!(
            first.value_kind()?,
            crate::descriptors::ValueDescriptorKind::BaseValue(base_types::BaseValueKind::String)
        );
        let constraints = first.effective_constraints()?;
        assert_eq!(constraints.len(), 3);
        assert_eq!(constraints[0].declared_on, parent);
        assert_eq!(constraints[1].member, constraints[2].member);
        assert_eq!(constraints[1].declared_on, target);
        assert_eq!(lineage_edge_reads(&target), before + 1);
        let result = registry.get_derived(&ids[0])?.expect("published derived result");
        assert!(!registry.is_empty()?);
        let allocated = owner.lookup().transient_count()?;
        let second = ValueDescriptor::from_holon(target.clone());
        assert_eq!(second.supported_operators()?.len(), 1);
        assert!(!second.is_array()?);
        assert_eq!(second.value_kind()?, first.value_kind()?);
        assert!(second.supports_operator(&second.supported_operators()?[0])?);
        assert!(second.supports_operator_by_name(MapString("derived-test-5".into()))?);
        assert!(second.get_operator_by_name(MapString("derived-test-5".into())).is_ok());

        assert_eq!(second.effective_validation_bindings()?.len(), 1);
        assert_eq!(first.effective_constraints()?.len(), 3);
        assert_eq!(lineage_edge_reads(&target), before + 1);
        assert_eq!(owner.lookup().transient_count()?, allocated);
        // Property wrappers consume the same products rather than another inheritance engine.
        let property = PropertyDescriptor::from_holon(target.clone());
        assert_eq!(property.effective_constraints()?.len(), 3);
        assert_eq!(property.effective_validation_bindings()?.len(), 1);
        assert_eq!(lineage_edge_reads(&target), before + 1);
        let projected_count = registry.capture(|context, _| context.lookup().transient_count())?;
        assert_eq!(projected_count, Some(allocated));
        assert_eq!(lineage_edge_reads(&target), before + 1);
        assert_eq!(first.instance_type_kind()?, Some(parent.clone()));
        crate::descriptors::test_support::replace_saved_relationship(
            &context,
            ids[1].clone(),
            R::Constraints.to_relationship_name(),
            vec![ids[3].clone()],
        );
        assert_eq!(first.effective_constraints()?[0].member.holon_id()?, ids[2]);
        let old_session = space.session()?;
        space.reset_session()?;
        assert!(matches!(old_session.check_active(), Err(HolonError::ObsoleteSession)));
        assert!(matches!(result.value_kind(), Err(HolonError::ObsoleteSession)));
        assert!(matches!(result.clone().lineage(), Err(HolonError::ObsoleteSession)));
        assert!(matches!(result.instance_type_kind(), Err(HolonError::ObsoleteSession)));
        assert!(registry.get_derived(&ids[0])?.is_none());
        // Registry publication begun before reset cannot resurrect an old product.
        assert!(matches!(
            registry.publish_derived(ids[0].clone(), &result, &old_session),
            Err(HolonError::ObsoleteSession)
        ));
        let old_lineage_reads = lineage_edge_reads(&target);
        assert_eq!(first.effective_constraints()?.len(), 3);
        assert_eq!(second.effective_validation_bindings()?.len(), 1);
        assert_eq!(property.effective_constraints()?.len(), 3);
        assert_eq!(lineage_edge_reads(&target), old_lineage_reads + 1);
        assert_eq!(first.effective_constraints()?[0].member.holon_id()?, ids[3]);
        context.dispose()?;
        assert_eq!(first.effective_constraints()?.len(), 3);
        drop(registry);
        assert!(first.effective_constraints().is_err());
        Ok(())
    }
    #[test]
    fn reset_during_capture_retries_without_shipping_the_old_owner_state() -> Result<(), HolonError>
    {
        let (registry, caller, descriptor, target) = fixture()?;
        registry.publish(target, &descriptor)?;
        let space = caller.space_manager();
        let mut once = true;
        let result = registry.capture_with_checkpoint(
            &mut |context, root| Ok((context.tx_id(), root.clone())),
            &mut || {
                if once {
                    once = false;
                    space.reset_session().unwrap();
                }
            },
        )?;
        assert!(result.is_none());
        assert!(caller.is_open());
        Ok(())
    }
    #[test]
    fn publication_during_capture_retries_the_registry_selection() -> Result<(), HolonError> {
        let (registry, _, descriptor, target) = fixture()?;
        registry.publish(target.clone(), &descriptor)?;
        let replacement = registry.context()?.clone_holon(descriptor.holon())?;
        replacement.mark_as_immutable()?;
        let replacement = EffectiveHolonDescriptor::from_constructed_holon(replacement.into());
        let mut projections = 0;
        let mut once = true;
        registry.capture_with_checkpoint(
            &mut |_, _| {
                projections += 1;
                Ok(())
            },
            &mut || {
                if once {
                    once = false;
                    registry.publish(target.clone(), &replacement).unwrap();
                }
            },
        )?;
        assert_eq!(projections, 2);
        assert_eq!(registry.get(&target)?.unwrap().holon(), replacement.holon());
        Ok(())
    }

    #[test]
    fn a_saved_source_does_not_admit_a_product_with_volatile_dependencies() -> Result<(), HolonError>
    {
        let (registry, caller, _, target) = fixture()?;
        let owner = registry.context()?;
        let candidate = owner.mutation().new_holon(None)?;
        let source = HolonReference::smart_from_id(caller.space_read_handle(), target.clone());
        let mut raw = owner.mutation().new_holon(None)?;
        raw.add_related_references(
            CoreRelationshipTypeName::DerivedLineage,
            vec![source, candidate.into()],
        )?;
        raw.mark_as_immutable()?;
        let result = DerivedDescriptor::from_constructed_holon(raw.into());
        let session = caller.space_manager().session()?;
        assert!(matches!(
            registry.publish_derived(target.clone(), &result, &session),
            Err(HolonError::InvalidParameter(_))
        ));
        assert!(registry.get_derived(&target)?.is_none());
        Ok(())
    }
}
