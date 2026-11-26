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
        let tx_id = super::registry_owner::validate_registry_owner(&self.owner, agent)?;
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
        let declaration: HolonReference = holons_core::SmartReference::new_with_properties(
            owner.space_read_handle(),
            HolonId::Local(LocalId(vec![99; 39])),
            core_types::PropertyMap::from([(
                core_types::PropertyName("Key".into()),
                base_types::BaseValue::StringValue("Example".into()),
            )]),
        )
        .into();
        ehd.add_related_holons("EffectiveInstanceRelationships", vec![declaration.clone()])?;
        let prepared = holons_core::descriptors::PreparedSchema {
            instance_deletion_allowed: Some(Ok(false)),
            relationships: vec![holons_core::descriptors::PreparedRelationship {
                identity: declaration.full_reference_id_string(),
                name: RelationshipName("Example".into()),
                policy: holons_core::RelationshipCachePolicy::Reuse,
                updateable: true,
            }],
            lineage: vec![target_id.clone()],
            origins: vec![
                vec![],
                vec![(target_id.clone(), 0)],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ],
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
            assert!(!ehd.instance_deletion_allowed()?);
            assert_eq!(ehd.declared_relationships()?.len(), 1);
            assert_eq!(
                ehd.get_declared_relationship_by_name("Example")?.holon().holon_id()?,
                HolonId::Local(LocalId(vec![99; 39]))
            );
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
            visualizer_evaluations: None,
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
    fn evaluation_fixture() -> Result<
        (
            Arc<holons_core::dahn::evaluation_registry::VisualizerEvaluationRegistry>,
            Arc<TransactionContext>,
            holons_core::dahn::composition::CompositionRequest,
            holons_core::dahn::composition::ResolvedComposition,
            Arc<HolonSpaceManager>,
        ),
        HolonError,
    > {
        use holons_core::dahn::{composition::*, CompositionSubject};
        let space = test_space();
        let agent = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let saved = |version| -> HolonReference {
            let mut properties = core_types::PropertyMap::from([(
                core_types::PropertyName("Key".into()),
                base_types::BaseValue::StringValue(format!("saved-{version}").into()),
            )]);
            if version == 6 {
                for (name, value) in
                    [("CompositionSubject", "OwnerSubject"), ("VisualizerKind", "TitleBar")]
                {
                    properties.insert(
                        core_types::PropertyName(name.into()),
                        base_types::BaseValue::EnumValue(base_types::MapEnumValue(value.into())),
                    );
                }
            }
            holons_core::SmartReference::new_with_properties(
                agent.space_read_handle(),
                HolonId::Local(LocalId(vec![version; 39])),
                properties,
            )
            .into()
        };
        let mut subject = agent.mutation().new_holon(None)?;
        subject.add_related_holons("DescribedBy", vec![saved(2)])?;
        let subject: HolonReference = subject.into();
        let request = CompositionRequest {
            value_holder: None,
            owner: saved(1),
            subject: subject.clone(),
            theme: saved(3),
            demand_slot: None,
            collection_source: None,
        };
        let references = vec![subject, saved(1), saved(2), saved(3), saved(4), saved(5), saved(6)];
        let data = CompositionData {
            bindings: vec![ChildBinding {
                value_holder: None,
                parent: None,
                owner: 1,
                slot: 6,
                subject: 0,
                subject_descriptor: 2,
                subject_binding: CompositionSubject::OwnerSubject,
                role: "titleBar".into(),
                property_name: None,
                label: None,
                selected: 4,
                implementation: 5,
                usage: None,
                reason: SelectionReason::PrescribedDefault,
                governing_record: None,
                configuration: std::collections::BTreeMap::from([(
                    "format".into(),
                    base_types::BaseValue::StringValue("original".into()),
                )]),
                required_operators: vec!["Title".into()],
                offered_operators: vec!["Title".into()],
            }],
            theme: Some(3),
            metadata: (1..7)
                .map(|reference| CompositionMetadata {
                    reference,
                    key: Some(format!("exact-{reference}")),
                    identity: format!("id-{reference}"),
                    versioned_key: Some(format!("exact-{reference}@1")),
                    properties: serde_json::json!({}),
                })
                .collect(),
            populations: vec![CompositionPopulation {
                source: 1,
                relationship: "HasPersonalVisualizerPreference".into(),
                members: vec![],
            }],
            ..Default::default()
        };
        let registry = Arc::new(
            holons_core::dahn::evaluation_registry::VisualizerEvaluationRegistry::new(&space),
        );
        space.attach_visualizer_evaluations(&registry);
        Ok((registry, agent, request, ResolvedComposition { references, data }, space))
    }

    #[test]
    fn evaluation_rebinds_new_occurrences_and_reconstructs_exact_facts_after_retirement(
    ) -> Result<(), HolonError> {
        use crate::VisualizerEvaluationsWire;
        let (registry, agent, request, result, space) = evaluation_fixture()?;
        assert!(registry.retain(&request, &result, 0)?);
        let original_carrier = VisualizerEvaluationsWire::capture(&registry)?;
        let next = space.get_transaction_manager().open_public_transaction(space.clone())?;
        let mut occurrence = next.mutation().new_holon(None)?;
        occurrence.add_related_holons(
            "DescribedBy",
            vec![request.subject.holon_descriptor()?.holon().clone()],
        )?;
        let mut compatible = request.clone();
        compatible.subject = occurrence.into();
        agent.dispose()?;
        // Both explicit eviction and the descriptor Session reset preserve the recipe.
        registry.evict_projections()?;
        next.space_read_handle().reset_session()?;
        let restored = registry.acquire(&next, &compatible)?.expect("compatible occurrence");
        assert_eq!(restored.references[restored.data.bindings[0].subject], compatible.subject);
        assert_eq!(restored.references[4], result.references[4]);
        assert_eq!(restored.references[5], result.references[5]);
        assert_eq!(restored.data.bindings[0].configuration, result.data.bindings[0].configuration);
        assert_eq!(
            restored.data.bindings[0].required_operators,
            result.data.bindings[0].required_operators
        );
        assert_eq!(restored.data.metadata, result.data.metadata);
        assert_eq!(restored.data.populations, result.data.populations);
        assert_eq!(restored.data.work.selections, 0);
        assert_eq!(restored.data.work.personal_population_requests, 0);
        let rebuilt_carrier = VisualizerEvaluationsWire::capture(&registry)?;
        assert_ne!(original_carrier.owner.get_tx_id(), rebuilt_carrier.owner.get_tx_id());
        assert_ne!(
            crate::TransientReferenceWire::new(
                original_carrier.owner.get_tx_id().unwrap(),
                original_carrier.root
            ),
            crate::TransientReferenceWire::new(
                rebuilt_carrier.owner.get_tx_id().unwrap(),
                rebuilt_carrier.root
            )
        );
        Ok(())
    }

    #[test]
    fn reconsideration_is_scoped_and_late_carriers_cannot_publish() -> Result<(), HolonError> {
        use crate::VisualizerEvaluationsWire;
        let (registry, agent, request, result, _space) = evaluation_fixture()?;
        registry.retain(&request, &result, 0)?;
        let mut other = request.clone();
        other.owner = HolonReference::smart_from_id(
            agent.space_read_handle(),
            HolonId::Local(LocalId(vec![99; 39])),
        );
        registry.retain(&other, &result, 0)?;
        let late = VisualizerEvaluationsWire::capture(&registry)?.bind(&agent)?;
        registry.reconsider(&request)?;
        assert!(registry.acquire(&agent, &request)?.is_none());
        assert!(registry.acquire(&agent, &other)?.is_some());
        assert!(matches!(registry.adopt(&late), Err(HolonError::InvalidState(_))));
        assert!(matches!(registry.retain(&request, &result, 0), Err(HolonError::InvalidState(_))));
        assert!(registry.acquire(&agent, &request)?.is_none());
        Ok(())
    }

    #[test]
    fn evaluation_carrier_is_separate_from_descriptor_and_public_pools() -> Result<(), HolonError> {
        use crate::{TransientReferenceWire, VisualizerEvaluationsWire};
        let (registry, agent, request, result, _space) = evaluation_fixture()?;
        let (descriptors, _) = fixture()?;
        let descriptor_owner = descriptors.owner.get_tx_id().unwrap();
        let descriptors = descriptors.restore(&agent)?;
        registry.retain(&request, &result, 0)?;
        let carrier = VisualizerEvaluationsWire::capture(&registry)?;
        let owner = carrier.owner.get_tx_id().unwrap();
        assert_ne!(owner, agent.tx_id());
        assert_ne!(owner, descriptor_owner);
        assert_eq!(carrier.owner.get_transient_holons().holons.len(), 1);
        assert!(carrier.owner.get_staged_holons().holons.is_empty());
        assert!(TransientReferenceWire::new(owner, carrier.root.clone()).bind(&agent).is_err());
        assert!(agent
            .descriptor_registry()
            .unwrap()
            .reference_owner(owner, &carrier.root)
            .is_none());
        let mut wrong = carrier.clone();
        wrong.owner = SessionStateWire::from_context(&agent)?;
        assert!(wrong.bind(&agent).is_err());
        let mut wrong = carrier.clone();
        wrong.owner = DescriptorRegistryWire::capture(&descriptors)?.unwrap().owner;
        assert!(wrong.bind(&agent).is_err());
        let bound = carrier.bind(&agent)?;
        assert!(bound.acquire(&agent, &request)?.is_some());
        Ok(())
    }
    #[test]
    fn covered_slot_demand_reuses_hierarchy_until_that_demand_is_explicitly_reconsidered(
    ) -> Result<(), HolonError> {
        let (registry, agent, request, result, _space) = evaluation_fixture()?;
        registry.retain(&request, &result, 0)?;
        let mut demand = request.clone();
        demand.demand_slot = Some(result.references[6].clone());
        let acquired = registry.acquire(&agent, &demand)?.expect("covered Slot");
        assert_eq!(acquired.data.bindings.len(), 1);
        assert_eq!(acquired.data.work.selections, 0);
        registry.reconsider(&demand)?;
        assert!(registry.acquire(&agent, &demand)?.is_none());
        assert!(
            registry.acquire(&agent, &request)?.is_some(),
            "another established scope remains intact"
        );
        Ok(())
    }
    #[test]
    fn column_scope_distinguishes_value_types_and_reuses_compatible_property_occurrences(
    ) -> Result<(), HolonError> {
        let (registry, agent, mut request, mut result, _space) = evaluation_fixture()?;
        let saved = |id| -> HolonReference {
            holons_core::SmartReference::new_with_properties(
                agent.space_read_handle(),
                HolonId::Local(LocalId(vec![id; 39])),
                core_types::PropertyMap::from([(
                    core_types::PropertyName("Key".into()),
                    base_types::BaseValue::StringValue(format!("type-{id}").into()),
                )]),
            )
            .into()
        };
        let property = |value_type| -> Result<HolonReference, HolonError> {
            let mut property = agent.mutation().new_holon(None)?;
            property.add_related_holons("DescribedBy", vec![saved(2)])?;
            property.add_related_holons("ValueType", vec![saved(value_type)])?;
            Ok(property.into())
        };
        let slot: HolonReference = holons_core::SmartReference::new_with_properties(
            agent.space_read_handle(),
            HolonId::Local(LocalId(vec![7; 39])),
            core_types::PropertyMap::from([
                (
                    core_types::PropertyName("Key".into()),
                    base_types::BaseValue::StringValue("column-slot".into()),
                ),
                (
                    core_types::PropertyName("CompositionSubject".into()),
                    base_types::BaseValue::EnumValue(base_types::MapEnumValue(
                        "OwnerSubject".into(),
                    )),
                ),
                (
                    core_types::PropertyName("VisualizerKind".into()),
                    base_types::BaseValue::EnumValue(base_types::MapEnumValue("Value".into())),
                ),
            ]),
        )
        .into();
        request.subject = property(11)?;
        request.demand_slot = Some(slot.clone());
        result.references[0] = request.subject.clone();
        result.references[2] = saved(11);
        result.references[6] = slot;
        registry.retain(&request, &result, 0)?;
        request.subject = property(12)?;
        assert!(
            registry.acquire(&agent, &request)?.is_none(),
            "a new ValueType is uncovered demand"
        );
        request.subject = property(11)?;
        let acquired = registry.acquire(&agent, &request)?.expect("same ValueType");
        assert_eq!(acquired.references[0], request.subject);
        assert_eq!(acquired.references[2], saved(11));
        assert_eq!(acquired.data.work.selections, 0);
        Ok(())
    }
}
