//! Commit-only transport for the Session registry's distinct owning transaction.
use super::SessionStateWire;
use core_types::{HolonError, TemporaryId};
use holons_core::core_shared_objects::{
    descriptor_registry::EffectiveHolonDescriptorRegistry, transactions::TransactionContext,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Carries prepared graphs under their original owner, never in the agent pool.
/// Runtime indexes are rebuilt at ingress; response products remain unpublished until reconciliation.
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
        self.restore_with_observer(agent, &mut |_| {})
    }

    /// Forwards clock-free restoration boundaries to the ingress adapter.
    pub fn restore_with_observer(
        self,
        agent: &Arc<TransactionContext>,
        observer: &mut impl FnMut(&'static str),
    ) -> Result<Arc<EffectiveHolonDescriptorRegistry>, HolonError> {
        self.restore_selected(agent, observer, true)
    }

    /// Binds a Commit return without publishing into the live host registry.
    pub fn bind_return(
        self,
        agent: &Arc<TransactionContext>,
    ) -> Result<
        Option<holons_core::core_shared_objects::descriptor_registry::ReturnedDescriptorProducts>,
        HolonError,
    > {
        self.restore_selected(agent, &mut |_| {}, false)?.returned_products()
    }

    fn restore_selected(
        self,
        agent: &Arc<TransactionContext>,
        observer: &mut impl FnMut(&'static str),
        install: bool,
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
        if install {
            EffectiveHolonDescriptorRegistry::restore_with_observer(
                agent,
                tx_id,
                self.root,
                |owner| self.owner.get_transient_holons().clone().bind(owner),
                observer,
            )
        } else {
            EffectiveHolonDescriptorRegistry::restore_detached(agent, tx_id, self.root, |owner| {
                self.owner.get_transient_holons().clone().bind(owner)
            })
        }
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
        fixture_for_version(77)
    }

    fn fixture_for_version(version: u8) -> Result<(DescriptorRegistryWire, HolonId), HolonError> {
        let space = test_space();
        let owner = space
            .get_transaction_manager()
            .open_private_restricted_cache_read_transaction_with_id(
                space.clone(),
                holons_core::core_shared_objects::transactions::TxId::from_str("400").unwrap(),
            )?;
        let target_id = HolonId::Local(LocalId(vec![version; 39]));
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
        ehd.add_related_holons("DescribedBy", vec![target.clone()])?;
        ehd.with_property_value("DerivedInstanceTypeKind", "{\"Ok\":null}")?;
        let prepared = holons_core::descriptors::PreparedSchema {
            lineage: vec![target_id.clone()],
            origins: vec![vec![]; 7],
            ..Default::default()
        };
        ehd.with_property_value("PreparedSchema", serde_json::to_string(&prepared).unwrap())?;
        ehd.add_related_holons("DescribedHolonType", vec![target.clone()])?;
        ehd.mark_as_immutable()?;
        let mut dd = owner.mutation().new_holon(None)?;
        dd.add_related_holons("DescribedBy", vec![target.clone()])?;
        dd.with_property_value("DerivedInstanceTypeKind", "{\"Ok\":null}")?;
        dd.with_property_value("DerivedRelationshipPolicy", "null")?;
        dd.add_related_holons("DerivedLineage", vec![target.clone()])?;
        let kind: Result<ValueDescriptorKind, HolonError> = Ok(ValueDescriptorKind::AnyBaseValue);
        dd.with_property_value("DerivedValueKind", serde_json::to_string(&kind).unwrap())?;
        dd.with_property_value("DerivedOrigins", "[[],[],[]]")?;
        dd.mark_as_immutable()?;
        let mut root = owner.mutation().new_holon(None)?;
        root.add_related_holons("DescribedBy", vec![target])?;
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
        let probe =
            holons_core::core_shared_objects::read_observations::CacheReadProbe::start(true);
        let mut phases = Vec::new();
        let registry = wire.restore_with_observer(&agent, &mut |phase| phases.push(phase))?;
        assert_eq!(phases, ["OwnerRestoration", "IndexRebuild", "Complete"]);
        assert!(probe.finish().is_empty(), "registry ingress must not read saved sources");
        assert_eq!(registry.population()?, (1, 1));
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
    #[test]
    fn returned_snapshots_are_unpublished_merge_both_partitions_and_preserve_live_entries(
    ) -> Result<(), HolonError> {
        let (wire, id) = fixture()?;
        let space = test_space();
        let agent = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let live = Arc::new(EffectiveHolonDescriptorRegistry::new(&space));
        space.attach_descriptor_registry(&live);
        let session = space.session()?;
        let returned = wire.clone().bind_return(&agent)?.unwrap();
        assert_eq!(live.population()?, (0, 0));
        assert_eq!(agent.lookup().transient_count()?, 0);
        live.merge_returned_products(returned.clone(), &session)?;
        assert_eq!(live.population()?, (1, 1));
        let selected = live.get(&id)?.unwrap().holon().clone();
        let owner = live.context()?;
        assert!(
            matches!(&selected, HolonReference::Transient(reference) if reference.tx_id() == owner.tx_id())
        );
        assert_ne!(owner.tx_id(), wire.owner.get_tx_id().unwrap());
        assert_eq!(
            live.get_derived(&id)?.unwrap().value_kind()?,
            ValueDescriptorKind::AnyBaseValue
        );
        let count = owner.lookup().transient_count()?;
        // A live publication for the same exact identity wins even a conflicting return.
        let mut replacement = owner.clone_holon(&selected)?;
        replacement.with_property_value("HostConcurrentResult", true)?;
        replacement.mark_as_immutable()?;
        live.publish(
            id.clone(),
            &EffectiveHolonDescriptor::from_constructed_holon(replacement.into()),
        )?;
        let concurrent = live.get(&id)?.unwrap().holon().clone();
        live.merge_returned_products(returned.clone(), &session)?;
        live.merge_returned_products(returned, &session)?;
        assert_eq!(live.get(&id)?.unwrap().holon(), &concurrent);
        assert_eq!(owner.lookup().transient_count()?, count + 1);
        assert_eq!(agent.lookup().transient_count()?, 0);
        // The next outbound snapshot restores and reuses the adopted products.
        let next_wire = DescriptorRegistryWire::capture(&live)?.unwrap();
        let next_space = test_space();
        let next_agent =
            next_space.get_transaction_manager().open_public_transaction(next_space.clone())?;
        let next = next_wire.restore(&next_agent)?;
        assert_eq!(next.population()?, (1, 1));
        assert_eq!(
            next.get(&id)?.unwrap().holon().full_reference_id_string(),
            concurrent.full_reference_id_string()
        );
        Ok(())
    }

    #[test]
    fn returned_snapshots_cannot_resurrect_a_retired_session() -> Result<(), HolonError> {
        let (wire, _) = fixture()?;
        let space = test_space();
        let agent = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let live = Arc::new(EffectiveHolonDescriptorRegistry::new(&space));
        space.attach_descriptor_registry(&live);
        let session = space.session()?;
        let returned = wire.bind_return(&agent)?.unwrap();
        assert_eq!(
            live.merge_returned_products(returned.clone(), &Arc::new(Default::default())),
            Err(HolonError::ObsoleteSession)
        );
        space.reset_session()?;
        assert_eq!(
            live.merge_returned_products(returned, &session),
            Err(HolonError::ObsoleteSession)
        );
        assert_eq!(live.population()?, (0, 0));
        assert!(space.session()?.check_active().is_ok());
        Ok(())
    }

    #[test]
    fn malformed_return_never_partially_imports_or_replaces_live_owner() -> Result<(), HolonError> {
        use crate::context_binding::HolonWire;
        let (wire, id) = fixture()?;
        for corruption in 0..10 {
            let space = test_space();
            let agent = space.get_transaction_manager().open_public_transaction(space.clone())?;
            let live = Arc::new(EffectiveHolonDescriptorRegistry::new(&space));
            space.attach_descriptor_registry(&live);
            let mut broken = wire.clone();
            let result_id = broken
                .owner
                .get_transient_holons()
                .holons
                .keys()
                .find(|id| **id != broken.root)
                .unwrap()
                .clone();
            if corruption == 0 {
                broken.owner.get_transient_holons_mut().holons.remove(&result_id);
            } else if corruption == 1 {
                broken.owner.set_tx_id(agent.tx_id());
            } else if corruption == 2 {
                broken.owner.set_restricted_cache_read(false);
            } else {
                let owner_tx_id = broken.owner.get_tx_id().unwrap().value();
                let root_id = broken.root.clone();
                let result =
                    broken.owner.get_transient_holons_mut().holons.get_mut(&result_id).unwrap();
                let mut value = serde_json::to_value(&*result).unwrap();
                match corruption {
                    3 => value["Transient"]["holon_state"] = serde_json::json!("Mutable"),
                    4 => {
                        value["Transient"]["transient_relationships"]["map"]["DescribedBy"]
                            ["members"] = serde_json::json!([])
                    }
                    5 => value["Transient"]["property_map"] = serde_json::json!({}),
                    6 | 7 => {
                        let phase = if corruption == 6 { "Transient" } else { "Staged" };
                        value["Transient"]["transient_relationships"]["map"]["DescribedBy"]
                            ["members"] =
                            serde_json::json!([{(phase): {"tx_id": owner_tx_id, "id": root_id}}]);
                    }
                    8 => {
                        value["Transient"]["transient_relationships"]["map"]["DescribedBy"]
                            ["state"] = serde_json::json!("Transient")
                    }
                    _ => {
                        value["Transient"]["property_map"]["PreparedSchema"] =
                            serde_json::json!({"StringValue":"malformed metadata"})
                    }
                }
                *result = serde_json::from_value::<HolonWire>(value).unwrap();
            }
            assert!(broken.bind_return(&agent).is_err(), "corruption {corruption}");
            assert_eq!(live.population()?, (0, 0));
            assert!(live.get(&id)?.is_none());
            assert_eq!(agent.lookup().transient_count()?, 0);
            // A valid return still works after each failure and uses the live owner.
            live.merge_returned_products(
                wire.clone().bind_return(&agent)?.unwrap(),
                &space.session()?,
            )?;
            assert_eq!(live.population()?, (1, 1));
        }
        Ok(())
    }

    #[test]
    fn merging_exact_versions_preserves_both_products_despite_transport_id_collisions(
    ) -> Result<(), HolonError> {
        let space = test_space();
        let agent = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let registry = Arc::new(EffectiveHolonDescriptorRegistry::new(&space));
        space.attach_descriptor_registry(&registry);
        let session = space.session()?;
        let (first, first_id) = fixture_for_version(77)?;
        let (next, next_id) = fixture_for_version(78)?;
        assert_eq!(first.root, next.root);
        registry.merge_returned_products(first.bind_return(&agent)?.unwrap(), &session)?;
        let original = registry.get(&first_id)?.unwrap().holon().clone();
        registry.merge_returned_products(next.bind_return(&agent)?.unwrap(), &session)?;
        assert_eq!(registry.population()?, (2, 2));
        assert_eq!(registry.get(&first_id)?.unwrap().holon(), &original);
        assert_ne!(registry.get(&next_id)?.unwrap().holon(), &original);
        assert!(registry.get_derived(&first_id)?.is_some());
        assert!(registry.get_derived(&next_id)?.is_some());
        Ok(())
    }
    #[test]
    fn optional_registry_return_roundtrips_in_actual_zome_encoding() -> Result<(), HolonError> {
        use crate::envelopes::{DanceEnvelopeResponse, DanceResponseEnvelope};
        let (wire, _) = fixture()?;
        let envelope = DanceResponseEnvelope {
            descriptor_registry: Some(wire),
            session: None,
            response: DanceEnvelopeResponse::Reference {
                result: Err(HolonError::CommitFailure("authoritative outcome".into())),
            },
        };
        let bytes = holochain_serialized_bytes::encode(&envelope).unwrap();
        let decoded: DanceResponseEnvelope = holochain_serialized_bytes::decode(&bytes).unwrap();
        assert_eq!(decoded, envelope);
        Ok(())
    }
}
