//! Commit-only transport for the Session registry's distinct owning transaction.
use super::SessionStateWire;
use core_types::{HolonError, TemporaryId};
use holons_core::core_shared_objects::{
    descriptor_registry::EffectiveHolonDescriptorRegistry, transactions::TransactionContext,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Carries prepared graphs under their original owner, never in the agent pool.
/// Runtime indexes and retirement tokens are rebuilt in the receiving guest space.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DescriptorRegistryWire {
    pub root: TemporaryId,
    pub owner: SessionStateWire,
}

impl DescriptorRegistryWire {
    /// Projects the current selected registry and its owner without changing pool ownership.
    pub fn capture(
        registry: &EffectiveHolonDescriptorRegistry,
    ) -> Result<Option<Self>, HolonError> {
        registry.capture(|owner, root| {
            Ok(Self { root: root.clone(), owner: SessionStateWire::from_context(owner)? })
        })
    }

    /// Validates transport ownership before binding references and indexing the root.
    pub fn restore(
        self,
        agent: &Arc<TransactionContext>,
    ) -> Result<Arc<EffectiveHolonDescriptorRegistry>, HolonError> {
        if !self.owner.is_restricted_cache_read()
            || self.owner.is_bootstrap_provisioning()
            || !self.owner.get_staged_holons().holons.is_empty()
            || self
                .owner
                .get_transient_holons()
                .holons
                .values()
                .any(|holon| !matches!(holon, crate::context_binding::HolonWire::Transient(_)))
        {
            return Err(HolonError::InvalidParameter(
                "Registry owner must be restricted, non-provisioning and unstaged".into(),
            ));
        }
        let expected_space = agent.get_space_holon()?.map(crate::HolonReferenceWire::from);
        if self.owner.get_local_space_holon_wire() != expected_space {
            return Err(HolonError::InvalidParameter(
                "Registry owner belongs to a different HolonSpace".into(),
            ));
        }
        let tx_id = self.owner.get_tx_id().ok_or_else(|| {
            HolonError::InvalidParameter(
                "Registry owner is missing its transaction identity".into(),
            )
        })?;
        EffectiveHolonDescriptorRegistry::restore(agent, tx_id, self.root, |owner| {
            self.owner.get_transient_holons().clone().bind(owner)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{HolonId, LocalId, RelationshipName};
    use holons_core::{
        core_shared_objects::{space_manager::HolonSpaceManager, ServiceRoutingPolicy},
        descriptors::{
            Descriptor, EffectiveHolonDescriptor, PropertyDescriptor, ValueDescriptor,
            ValueDescriptorKind,
        },
        HolonReference, ReadableHolon, WritableHolon,
    };
    #[derive(Debug)]
    struct UnusedHolonService;

    impl holons_core::HolonServiceApi for UnusedHolonService {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn commit_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: &[holons_core::StagedReference],
        ) -> Result<holons_core::TransientReference, HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn delete_holon_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: &LocalId,
        ) -> Result<(), HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn fetch_all_related_holons_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: &core_types::HolonId,
        ) -> Result<holons_core::RelationshipMap, HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn fetch_holon_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: &core_types::HolonId,
        ) -> Result<holons_core::core_shared_objects::Holon, HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn fetch_related_holons_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: &core_types::HolonId,
            _: &RelationshipName,
        ) -> Result<holons_core::HolonCollection, HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn get_all_holons_internal(
            &self,
            _: &Arc<TransactionContext>,
        ) -> Result<holons_core::HolonCollection, HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn load_holons_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: holons_core::TransientReference,
        ) -> Result<holons_core::TransientReference, HolonError> {
            panic!("wire binding must not access persistence")
        }
    }

    fn test_space() -> Arc<HolonSpaceManager> {
        HolonSpaceManager::new_with_managers(
            None,
            Arc::new(UnusedHolonService),
            None,
            ServiceRoutingPolicy::BlockExternal,
        )
    }

    fn fixture() -> Result<(DescriptorRegistryWire, HolonId), HolonError> {
        let space = test_space();
        let owner = space
            .get_transaction_manager()
            .open_private_restricted_cache_read_transaction_with_id(
                space.clone(),
                holons_core::core_shared_objects::transactions::TxId::from_str("400").unwrap(),
            )?;
        let target_id = HolonId::Local(LocalId(vec![77; 39]));
        let target: HolonReference = holons_core::SmartReference::new_with_properties(
            owner.space_read_handle(),
            target_id.clone(),
            core_types::PropertyMap::from([(
                core_types::PropertyName("Key".into()),
                base_types::BaseValue::StringValue("SavedDescriptor".into()),
            )]),
        )
        .into();
        let mut ehd = owner.mutation().new_holon(None)?;
        ehd.add_related_holons("DescribedHolonType", vec![target.clone()])?;
        ehd.mark_as_immutable()?;
        let mut dd = owner.mutation().new_holon(None)?;
        dd.add_related_holons("DerivedLineage", vec![target])?;
        let kind: Result<ValueDescriptorKind, HolonError> = Ok(ValueDescriptorKind::AnyBaseValue);
        dd.with_property_value("DerivedValueKind", serde_json::to_string(&kind).unwrap())?;
        dd.with_property_value("DerivedOrigins", "[[],[]]")?;
        dd.mark_as_immutable()?;
        let mut root = owner.mutation().new_holon(None)?;
        root.add_related_holons("PreparedDescriptors", vec![ehd.into()])?;
        root.add_related_holons("PreparedDerivedDescriptors", vec![dd.into()])?;
        let wire = DescriptorRegistryWire {
            root: root.temporary_id(),
            owner: SessionStateWire::from_context(&owner)?,
        };
        // The wire graph must be independent of the source runtime's lifetime.
        let wire = serde_json::from_str(&serde_json::to_string(&wire).unwrap()).unwrap();
        owner.dispose()?;
        Ok((wire, target_id))
    }

    #[test]
    fn ordinary_saved_accessors_reuse_received_registry_without_storage_or_construction(
    ) -> Result<(), HolonError> {
        let (wire, id) = fixture()?;
        let space = test_space();
        let agent = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let registry = wire.restore(&agent)?;
        let owner = registry.context()?;
        assert_eq!(owner.tx_id().value(), 400);
        assert_ne!(owner.tx_id(), agent.tx_id());
        let count = owner.lookup().transient_count()?;
        let saved = HolonReference::smart_from_id(agent.space_read_handle(), id.clone());
        for _ in 0..3 {
            let ehd = EffectiveHolonDescriptor::acquire(saved.clone())?;
            assert_eq!(ehd.described_holon_type()?.holon().holon_id()?, id);
            assert_eq!(
                ehd.holon().full_reference_id_string(),
                registry.get(&id)?.unwrap().holon().full_reference_id_string()
            );
            assert_eq!(
                ValueDescriptor::from_holon(saved.clone()).value_kind()?,
                ValueDescriptorKind::AnyBaseValue
            );
            assert!(PropertyDescriptor::from_holon(saved.clone())
                .effective_constraints()?
                .is_empty());
        }
        assert_eq!(owner.lookup().transient_count()?, count);
        assert_eq!(agent.lookup().transient_count()?, 0);
        assert!(space.get_transaction_manager().get_transaction(&owner.tx_id())?.is_none());
        let captured = DescriptorRegistryWire::capture(&registry)?.unwrap();
        assert_eq!(captured.owner.get_tx_id(), Some(owner.tx_id()));
        assert_eq!(agent.lookup().transient_count()?, 0);
        let held = registry.get_derived(&id)?.unwrap();
        agent.space_read_handle().reset_session()?;
        assert!(matches!(held.value_kind(), Err(HolonError::ObsoleteSession)));
        let next_space = test_space();
        let next_agent =
            next_space.get_transaction_manager().open_public_transaction(next_space.clone())?;
        let received = captured.restore(&next_agent)?;
        let target = HolonReference::smart_from_id(next_agent.space_read_handle(), id);
        assert_eq!(
            ValueDescriptor::from_holon(target).value_kind()?,
            ValueDescriptorKind::AnyBaseValue
        );
        assert_eq!(received.context()?.tx_id(), owner.tx_id());
        Ok(())
    }

    #[test]
    fn registry_ingress_rejects_missing_results_and_wrong_owner_without_installing(
    ) -> Result<(), HolonError> {
        let (wire, _) = fixture()?;
        for corruption in 0..4 {
            let space = test_space();
            let agent = space.get_transaction_manager().open_public_transaction(space.clone())?;
            let mut broken = wire.clone();
            match corruption {
                0 => {
                    broken.owner.get_transient_holons_mut().holons.remove(&broken.root);
                }
                1 => {
                    let id = broken
                        .owner
                        .get_transient_holons()
                        .holons
                        .keys()
                        .find(|id| **id != broken.root)
                        .unwrap()
                        .clone();
                    broken.owner.get_transient_holons_mut().holons.remove(&id);
                }
                2 => broken.owner.set_tx_id(agent.tx_id()),
                _ => broken.owner.set_restricted_cache_read(false),
            }
            assert!(broken.restore(&agent).is_err());
            assert!(agent.descriptor_registry().is_none());
            assert_eq!(agent.lookup().transient_count()?, 0);
        }
        Ok(())
    }
}
