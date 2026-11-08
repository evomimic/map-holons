//! Guest preparation, Session reuse, and Commit registry-owner transport.
use holons_core::descriptors::{ComputedEffectiveSchema, Descriptor, EffectiveHolonDescriptor};
use holons_prelude::prelude::*;
use holons_test::{init_test_runtime, DancesTestCase};
use map_commands_contract::{
    HolonAction, HolonCommand, MapCommand, MapResult, ReadableHolonAction, SpaceCommand,
};
use map_commands_runtime::ExecutionPolicy;

/// Models the guest ingress boundary without borrowing the host registry's ownership.
fn restore_registry_request(
    agent: &std::sync::Arc<holons_core::core_shared_objects::transactions::TransactionContext>,
    wire: holons_boundary::DescriptorRegistryWire,
) -> (
    std::sync::Arc<holons_core::core_shared_objects::transactions::TransactionContext>,
    std::sync::Arc<
        holons_core::core_shared_objects::descriptor_registry::EffectiveHolonDescriptorRegistry,
    >,
) {
    use holons_core::core_shared_objects::{
        space_manager::HolonSpaceManager, ServiceRoutingPolicy,
    };
    let space = HolonSpaceManager::new_with_managers(
        None,
        std::sync::Arc::new(holons_client::ClientHolonService::development_default()),
        agent.get_space_holon().unwrap().map(|reference| reference.holon_id().unwrap()),
        ServiceRoutingPolicy::Combined,
    );
    let context = space
        .get_transaction_manager()
        .open_public_transaction_with_id(space.clone(), agent.tx_id())
        .unwrap();
    let registry = wire.restore(&context).unwrap();
    (context, registry)
}

#[tokio::test(flavor = "multi_thread")]
async fn guest_construction_returns_a_retained_sealed_schema_graph() {
    let started = std::time::Instant::now();
    let (runtime, _) = init_test_runtime(&mut DancesTestCase::default()).await;
    let mut contexts = Vec::new();
    let mut references = Vec::new();
    for _ in 0..2 {
        let MapResult::TransactionCreated { tx_id } = runtime
            .execute_command(
                MapCommand::Space(SpaceCommand::BeginTransaction),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap()
        else {
            panic!("expected transaction")
        };
        let context = runtime.session().get_transaction(&tx_id).unwrap();
        let source = context.get_space_holon().unwrap().unwrap();
        let before = context.lookup().transient_count().unwrap();
        let MapResult::Reference(reference) = runtime
            .execute_command(
                MapCommand::Holon(HolonCommand {
                    context: context.clone(),
                    target: source,
                    action: HolonAction::Read(ReadableHolonAction::GetEffectiveDescriptor),
                }),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap()
        else {
            panic!("expected descriptor")
        };
        assert_eq!(context.lookup().transient_count().unwrap(), before);
        references.push(reference);
        contexts.push(context);
    }
    assert_eq!(references[0].full_reference_id_string(), references[1].full_reference_id_string());
    let descriptor = EffectiveHolonDescriptor::try_from_holon(references[0].clone()).unwrap();
    let singular = descriptor.singular_navigable_relationships().unwrap();
    let MapResult::Collection(plural_collection) = runtime
        .execute_command(
            MapCommand::Holon(HolonCommand {
                context: contexts[0].clone(),
                target: references[0].clone(),
                action: HolonAction::Read(ReadableHolonAction::GetPluralNavigableRelationships),
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("expected plural descriptor collection")
    };
    let plural = descriptor.plural_navigable_relationships().unwrap();
    assert_eq!(
        plural_collection
            .get_members()
            .iter()
            .map(|entry| entry.full_reference_id_string())
            .collect::<Vec<_>>(),
        plural.iter().map(|entry| entry.holon().full_reference_id_string()).collect::<Vec<_>>()
    );
    assert!(!singular.is_empty());
    assert!(!plural.is_empty());
    for relationship in &singular {
        assert_eq!(relationship.effective_cardinality().unwrap().maximum, Some(1));
    }
    for relationship in &plural {
        let maximum = relationship.effective_cardinality().unwrap().maximum;
        assert!(maximum.is_none() || maximum.is_some_and(|value| value > 1));
    }
    let identities = |entries: Vec<holons_core::RelationshipDescriptor>| {
        entries
            .into_iter()
            .map(|entry| entry.holon().full_reference_id_string())
            .collect::<Vec<_>>()
    };
    let expected = singular
        .iter()
        .chain(&plural)
        .map(|entry| entry.holon().full_reference_id_string())
        .collect::<Vec<_>>();
    assert_eq!(identities(descriptor.navigable_relationships().unwrap()), expected);
    for (is_singular, partition) in [(true, singular), (false, plural)] {
        let source = contexts[0].get_space_holon().unwrap().unwrap();
        let MapResult::QualifiedRelationships(available) = runtime
            .execute_command(
                MapCommand::Holon(HolonCommand {
                    context: contexts[0].clone(),
                    target: source,
                    action: HolonAction::Read(
                        ReadableHolonAction::GetAvailableRelationshipsFromEffective {
                            descriptor: references[0].clone(),
                            singular: Some(is_singular),
                        },
                    ),
                }),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap()
        else {
            panic!("expected relationship partition")
        };
        let expected = identities(partition);
        assert_eq!(
            available.len(),
            expected.len(),
            "saved sources admit both declared and inverse vocabulary"
        );
        assert!(available
            .iter()
            .all(|entry| expected.contains(&entry.descriptor.holon().full_reference_id_string())));
    }
    // Cardinality reads follow Constraints on relationship definitions. Those
    // structural collections must acquire policy after bootstrap as well.
    let relationship = descriptor
        .navigable_relationships()
        .unwrap()
        .into_iter()
        .find(|entry| {
            entry.base_relationship_name().unwrap()
                == CoreRelationshipTypeName::DescribedBy.to_relationship_name()
        })
        .expect("DescribedBy is navigable");
    let constraints =
        relationship.holon().related_holons(CoreRelationshipTypeName::Constraints).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(
            &constraints,
            &relationship.holon().related_holons(CoreRelationshipTypeName::Constraints).unwrap()
        ),
        "prepared structural policy must retain immutable Constraints membership"
    );
    let target = descriptor.described_holon_type().unwrap();
    let wrapper = holons_core::descriptors::HolonDescriptor::from_holon(target.holon().clone());
    let selected = wrapper.effective_descriptor().unwrap();
    assert_eq!(selected.holon(), descriptor.holon());
    let direct = ComputedEffectiveSchema::resolve(target.holon()).unwrap();
    let expected_kind = direct
        .lineage
        .iter()
        .find(|ancestor| {
            holons_core::descriptors::HolonDescriptor::from_holon((*ancestor).clone())
                .header()
                .defines_instance_type_kind()
                .unwrap()
        })
        .unwrap();
    assert_eq!(descriptor.instance_type_kind().unwrap().as_ref(), Some(expected_kind));
    assert_eq!(wrapper.instance_type_kind().unwrap().as_ref(), Some(expected_kind));
    let prepared = ComputedEffectiveSchema::from_prepared(&descriptor).unwrap();
    for source in [
        CoreRelationshipTypeName::InstanceProperties,
        CoreRelationshipTypeName::InstanceRelationships,
        CoreRelationshipTypeName::AffordsDance,
        CoreRelationshipTypeName::AffordsCommand,
        CoreRelationshipTypeName::ValidationBindings,
        CoreRelationshipTypeName::Constraints,
        CoreRelationshipTypeName::InstanceKeyRule,
    ] {
        let identities = |schema: &ComputedEffectiveSchema| {
            schema
                .contributions(source.clone())
                .iter()
                .map(|entry| {
                    (
                        entry.member.full_reference_id_string(),
                        entry.declared_on.full_reference_id_string(),
                        entry.local_position,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(identities(&direct), identities(&prepared));
    }
    let navigation = |schema: &ComputedEffectiveSchema| {
        schema
            .navigable_relationships()
            .unwrap()
            .iter()
            .map(|entry| {
                (
                    entry.member.full_reference_id_string(),
                    entry.declared_on.full_reference_id_string(),
                    entry.local_position,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        navigation(&direct),
        navigation(&prepared),
        "presentation partition order must not scramble structural provenance"
    );
    assert!(descriptor.contributions().unwrap().is_empty());
    assert!(!descriptor.navigable_relationships().unwrap().is_empty());
    let mut sealed = references[0].clone();
    assert!(sealed.with_property_value("Corrupt", true).is_err());
    // Concurrent consumers share one preparation and retain no construction envelopes.
    let implementation: HolonReference = contexts[0]
        .lookup()
        .get_saved_holon_by_key(&MapString(
            "ConstructEffectiveHolonDescriptor.DanceImplementation".into(),
        ))
        .unwrap()
        .into();
    let implementation_type = implementation.holon_descriptor().unwrap();
    let owner = runtime.session().descriptor_registry().context().unwrap();
    let already_prepared = runtime
        .session()
        .descriptor_registry()
        .get(&implementation_type.holon().holon_id().unwrap())
        .unwrap()
        .is_some();
    let before = owner.lookup().transient_count().unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let calls = (0..4)
        .map(|_| {
            let subject = implementation.clone();
            let barrier = barrier.clone();
            tokio::task::spawn_blocking(move || {
                barrier.wait();
                subject.effective_descriptor().unwrap().holon().full_reference_id_string()
            })
        })
        .collect::<Vec<_>>();
    let mut shared = Vec::new();
    for call in calls {
        shared.push(call.await.unwrap());
    }
    assert!(shared.iter().all(|identity| identity == &shared[0]));
    assert_eq!(owner.lookup().transient_count().unwrap() - before, i64::from(!already_prepared));

    let agent_count = contexts[0].lookup().transient_count().unwrap();
    let wire =
        holons_boundary::DescriptorRegistryWire::capture(runtime.session().descriptor_registry())
            .unwrap()
            .unwrap();
    let (snapshot, _received_registry) = restore_registry_request(&contexts[0], wire);
    assert_eq!(contexts[0].lookup().transient_count().unwrap(), agent_count);
    let received_target = HolonReference::smart_from_id(
        snapshot.space_read_handle(),
        target.holon().holon_id().unwrap(),
    );
    let copy = EffectiveHolonDescriptor::acquire(received_target).unwrap();
    assert_eq!(
        copy.holon().full_reference_id_string(),
        descriptor.holon().full_reference_id_string()
    );
    for (_, collection) in copy.holon().all_related_holons().unwrap().iter() {
        assert!(collection
            .read()
            .unwrap()
            .get_members()
            .iter()
            .all(|member| matches!(member, HolonReference::Smart(_))));
    }
    // Agent graphs can reference a prepared result without taking ownership of it.
    // Guest ingress must install the registry before binding this cross-owner reference.
    let mut carrier = contexts[1].mutation().new_holon(None).unwrap();
    carrier.add_related_holons("PreparedResult", vec![references[0].clone()]).unwrap();
    let carrier_id = carrier.temporary_id();
    // Commit ships a separate registry-owner transaction; no received results enter the agent pool.
    let committed = contexts[1].commit().unwrap();
    let rebound_carrier =
        TransientReference::from_temporary_id(contexts[1].context_handle(), &carrier_id);
    let received = rebound_carrier.related_holons("PreparedResult").unwrap();
    assert_eq!(
        received.read().unwrap().get_members()[0].full_reference_id_string(),
        references[0].full_reference_id_string()
    );
    assert_eq!(
        committed.property_value(CorePropertyTypeName::CommitRequestStatus).unwrap(),
        Some(BaseValue::StringValue("Complete".into()))
    );
    let committed_copy =
        EffectiveHolonDescriptor::find_in_context(&contexts[1], target.holon()).unwrap().unwrap();
    assert_eq!(
        committed_copy.instance_properties().unwrap().len(),
        descriptor.instance_properties().unwrap().len()
    );
    assert!(descriptor.instance_properties().is_ok());

    let pulled = descriptor.pull().unwrap();
    assert_ne!(
        pulled.holon().full_reference_id_string(),
        descriptor.holon().full_reference_id_string()
    );
    assert!(matches!(descriptor.instance_properties(), Err(HolonError::ObsoleteSession)));
    assert!(matches!(selected.instance_properties(), Err(HolonError::ObsoleteSession)));
    assert_eq!(wrapper.effective_descriptor().unwrap().holon(), pulled.holon());
    assert_eq!(
        copy.instance_type_kind().unwrap().map(|kind| kind.holon_id().unwrap()),
        Some(expected_kind.holon_id().unwrap())
    );
    assert_eq!(pulled.described_holon_type().unwrap().holon(), target.holon());
    assert!(EffectiveHolonDescriptor::try_from_holon(references[0].clone()).is_err());

    let descriptor_owner = match &references[0] {
        HolonReference::Transient(reference) => reference.tx_id(),
        _ => panic!("transient descriptor"),
    };
    for context in contexts {
        runtime.session().dispose_transaction(&context.tx_id()).await.unwrap();
    }
    let owner_context = runtime.session().get_transaction(&descriptor_owner).unwrap();
    let retired_read = HolonCommand {
        context: owner_context.clone(),
        target: references[0].clone(),
        action: HolonAction::Read(ReadableHolonAction::GetRelatedHolons {
            name: CoreRelationshipTypeName::EffectiveInstanceProperties.to_relationship_name(),
            hint: holons_core::RelationshipReadHint::SchemaDefault,
        }),
    };
    assert!(matches!(
        runtime.execute_command(MapCommand::Holon(retired_read), ExecutionPolicy::default()).await,
        Err(HolonError::ObsoleteSession)
    ));
    let read = HolonCommand {
        context: owner_context.clone(),
        target: holons_boundary::HolonReferenceWire::from(pulled.holon())
            .bind(&owner_context)
            .unwrap(),
        action: HolonAction::Read(ReadableHolonAction::GetInstanceProperties),
    };
    assert!(matches!(
        runtime.execute_command(MapCommand::Holon(read), ExecutionPolicy::default()).await.unwrap(),
        MapResult::Collection(_)
    ));
    assert!(!pulled.instance_properties().unwrap().is_empty());
    eprintln!(
        "effective Holon descriptor: Session acquisition, reuse and snapshot completed in {:?}",
        started.elapsed()
    );
    drop(runtime);
    assert!(pulled.instance_properties().is_err());
}

/// The non-HolonType trial keeps the public wrapper API while reusing one
/// guest-produced, Session-retained result for each exact saved definition.
#[tokio::test(flavor = "multi_thread")]
async fn value_and_property_wrappers_reuse_guest_derived_products() {
    use holons_core::descriptors::{
        effective_relationship_targets, PropertyDescriptor, ValueDescriptor,
    };
    let (runtime, _) = init_test_runtime(&mut DancesTestCase::default()).await;
    let MapResult::TransactionCreated { tx_id } = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("expected transaction")
    };
    let context = runtime.session().get_transaction(&tx_id).unwrap();
    let registry = context.descriptor_registry().unwrap();
    let saved_value: HolonReference = context
        .lookup()
        .get_saved_holon_by_key(&MapString("MapStringValueType.StringValueType".into()))
        .unwrap()
        .into();
    let saved_property: HolonReference = context
        .lookup()
        .get_saved_holon_by_key(&MapString("TypeName.PropertyType".into()))
        .unwrap()
        .into();
    // ValueType must resolve on demand without a separately primed policy table.
    let first = saved_property.related_holons(CoreRelationshipTypeName::ValueType).unwrap();
    let probe = holons_core::core_shared_objects::read_observations::CacheReadProbe::start(true);
    let repeated = saved_property.related_holons(CoreRelationshipTypeName::ValueType).unwrap();
    let observations = probe.finish();
    assert!(std::sync::Arc::ptr_eq(&first, &repeated));
    assert!(observations.iter().any(|entry| entry.source == saved_property.holon_id().unwrap()
        && entry.relationship == Some(CoreRelationshipTypeName::ValueType.to_relationship_name())
        && entry.outcome == "hit"));
    assert!(!observations.iter().any(|entry| entry.outcome != "hit"));
    let value_type_relationship: HolonReference = context
        .lookup()
        .get_saved_holon_by_key(&MapString(
            "(PropertyType.TypeDescriptor)-[ValueType]->(ValueType.TypeDescriptor)".into(),
        ))
        .unwrap()
        .into();
    assert_eq!(
        holons_core::descriptors::RelationshipDescriptor::from_holon(value_type_relationship)
            .membership_cache_policy()
            .unwrap(),
        holons_core::RelationshipCachePolicy::Reuse
    );
    let incoming = saved_value.related_holons("ValueTypeFor").unwrap();
    assert!(!std::sync::Arc::ptr_eq(
        &incoming,
        &saved_value.related_holons("ValueTypeFor").unwrap()
    ));
    assert!(matches!(
        saved_property.related_holons("NoSuchRelationship"),
        Err(HolonError::DescriptorDeclarationNotFound { .. })
    ));
    let value = ValueDescriptor::from_holon(saved_value.clone());
    assert_eq!(
        value.value_kind().unwrap(),
        holons_core::descriptors::ValueDescriptorKind::BaseValue(base_types::BaseValueKind::String)
    );
    assert!(!value.is_array().unwrap());
    assert!(value.supported_operators().unwrap().len() >= 3);
    assert!(!value.effective_validation_bindings().unwrap().is_empty());
    let property = PropertyDescriptor::from_holon(saved_property.clone());
    let expected = effective_relationship_targets(
        &saved_property,
        CoreRelationshipTypeName::ValidationBindings,
    )
    .unwrap();
    let actual = property.effective_validation_bindings().unwrap();
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.member, expected.member);
        assert_eq!(actual.declared_on, expected.declared_on);
    }
    let value_result = registry.get_derived(&saved_value.holon_id().unwrap()).unwrap().unwrap();
    let property_result =
        registry.get_derived(&saved_property.holon_id().unwrap()).unwrap().unwrap();
    assert_ne!(value_result.lineage().unwrap()[0], property_result.lineage().unwrap()[0]);
    let owner = registry.context().unwrap();
    let before = owner.lookup().transient_count().unwrap();
    assert!(!ValueDescriptor::from_holon(saved_value.clone()).is_array().unwrap());
    assert!(
        ValueDescriptor::from_holon(saved_value.clone()).supported_operators().unwrap().len() >= 3
    );
    PropertyDescriptor::from_holon(saved_property.clone()).effective_constraints().unwrap();
    assert_eq!(owner.lookup().transient_count().unwrap(), before);
    use holons_trust_channel::envelopes::dance_envelope_adapter::DanceEnvelopeAdapter;
    let ordinary = DanceEnvelopeAdapter::build_request_envelope(
        &context,
        holon_dance_builders::build_get_all_holons_dance_request().unwrap(),
    )
    .unwrap();
    assert!(ordinary.descriptor_registry.is_none());
    let commit_envelope = DanceEnvelopeAdapter::build_request_envelope(
        &context,
        holon_dance_builders::build_commit_dance_request().unwrap(),
    )
    .unwrap();
    let wire = commit_envelope.descriptor_registry.unwrap();
    assert_eq!(wire.owner.get_tx_id(), Some(owner.tx_id()));
    assert_ne!(wire.owner.get_tx_id(), commit_envelope.session.as_ref().unwrap().get_tx_id());
    assert!(!commit_envelope
        .session
        .as_ref()
        .unwrap()
        .get_transient_holons()
        .holons
        .contains_key(&wire.root));
    let (snapshot, _received_registry) = restore_registry_request(&context, wire);
    let copied =
        holons_core::descriptors::DerivedDescriptor::find_in_context(&snapshot, &saved_value)
            .unwrap()
            .unwrap();
    assert_eq!(
        copied.supported_operators().unwrap().len(),
        value.supported_operators().unwrap().len()
    );
    assert_eq!(copied.value_kind().unwrap(), value.value_kind().unwrap());
    let old_kind = value.instance_type_kind().unwrap().map(|kind| kind.holon_id().unwrap());
    context.space_read_handle().reset_session().unwrap();
    assert!(matches!(value_result.value_kind(), Err(HolonError::ObsoleteSession)));
    assert!(matches!(property_result.effective_constraints(), Err(HolonError::ObsoleteSession)));
    // Received products remain readable after the host Session retires.
    assert_eq!(copied.value_kind().unwrap(), value.value_kind().unwrap());
    assert_eq!(old_kind, value.instance_type_kind().unwrap().map(|kind| kind.holon_id().unwrap()));
    assert_eq!(copied.instance_type_kind().unwrap().map(|kind| kind.holon_id().unwrap()), old_kind);
    assert!(property.effective_validation_bindings().is_ok());
    // A real, nonempty guest Commit validates using the transported registry and
    // preserves existing products when its persistence effects are descriptor-independent.
    let name_property: HolonReference = context
        .lookup()
        .get_saved_holon_by_key(&MapString("HolonSpaceName.PropertyType".into()))
        .unwrap()
        .into();
    PropertyDescriptor::from_holon(name_property).effective_validation_bindings().unwrap();
    let space_type: HolonReference = context
        .lookup()
        .get_saved_holon_by_key(&MapString("HolonSpace.HolonType".into()))
        .unwrap()
        .into();
    let holon_type = holons_core::descriptors::HolonDescriptor::from_holon(space_type.clone());
    let before_holon_commit = holon_type.effective_descriptor().unwrap();
    let mut created = context.mutation().new_holon(Some("SessionResetProof".into())).unwrap();
    created.with_descriptor(space_type).unwrap();
    created.with_property_value("HolonSpaceName", "SessionResetProof").unwrap();
    let mut staged_proof = context.mutation().stage_new_holon(created).unwrap();
    let before_commit = registry.get_derived(&saved_value.holon_id().unwrap()).unwrap().unwrap();
    let rejected = context.commit().unwrap();
    assert_eq!(
        rejected.property_value(CorePropertyTypeName::CommitRequestStatus).unwrap(),
        Some(BaseValue::StringValue("Rejected".into()))
    );
    assert!(context.is_open());
    assert!(before_commit.value_kind().is_ok());
    assert!(before_holon_commit.instance_properties().is_ok());
    // HolonSpace requires all three properties. Correct the rejected candidate
    // in place; failed validation must preserve both staged work and the Session.
    staged_proof.with_property_value("DisplayName", "Session reset proof").unwrap();
    staged_proof.with_property_value("Description", "Checks local Commit adoption").unwrap();
    let session_before_commit = context.space_read_handle().session().unwrap();
    let committed = context.commit().unwrap();
    assert_eq!(
        committed.property_value(CorePropertyTypeName::CommitRequestStatus).unwrap(),
        Some(BaseValue::StringValue("Complete".into())),
        "Commit findings: {:?}",
        staged_proof.validation_findings().unwrap()
    );
    assert_eq!(
        committed.property_value(CorePropertyTypeName::DescriptorResetRequired).unwrap(),
        Some(BaseValue::BooleanValue(false.into())),
    );
    assert!(session_before_commit.check_active().is_ok());
    assert!(before_commit.value_kind().is_ok());
    assert!(before_holon_commit.instance_properties().is_ok());
    let allocated = owner.lookup().transient_count().unwrap();
    assert_eq!(
        ValueDescriptor::from_holon(saved_value.clone()).value_kind().unwrap(),
        copied.value_kind().unwrap()
    );
    assert_eq!(holon_type.effective_descriptor().unwrap().holon(), before_holon_commit.holon());
    assert_eq!(owner.lookup().transient_count().unwrap(), allocated);

    // A new instance version initializes DescribedBy on a new exact identity too.
    let update_tx = runtime.session().begin_transaction().await.unwrap();
    let update_context = runtime.session().get_transaction(&update_tx).unwrap();
    let mut updated = update_context
        .mutation()
        .stage_new_version_from_id(staged_proof.holon_id().unwrap())
        .unwrap();
    updated.with_property_value("Description", "Updated instance without schema effects").unwrap();
    let versioned = update_context.commit().unwrap();
    assert_eq!(
        versioned.property_value(CorePropertyTypeName::CommitRequestStatus).unwrap(),
        Some(BaseValue::StringValue("Complete".into())),
        "{:?}",
        updated.validation_findings().unwrap()
    );
    assert_eq!(
        versioned.property_value(CorePropertyTypeName::DescriptorResetRequired).unwrap(),
        Some(BaseValue::BooleanValue(false.into()))
    );
    assert!(session_before_commit.check_active().is_ok());
    assert_eq!(holon_type.effective_descriptor().unwrap().holon(), before_holon_commit.holon());

    // A non-definitional relationship writes an inverse onto a saved Schema,
    // but neither physical collection is a prepared descriptor input.
    let graph_tx = runtime.session().begin_transaction().await.unwrap();
    let graph_context = runtime.session().get_transaction(&graph_tx).unwrap();
    let mut graph_update =
        graph_context.mutation().stage_new_version_from_id(updated.holon_id().unwrap()).unwrap();
    let available_schema: HolonReference = graph_context
        .lookup()
        .get_saved_holon_by_key(&MapString("MAP Core Schema-v0.0.7".into()))
        .unwrap()
        .into();
    graph_update.add_related_holons("AvailableSchemas", vec![available_schema]).unwrap();
    assert!(graph_update
        .is_in_state(
            &graph_context,
            holons_core::core_shared_objects::holon::StagedState::ForUpdateGraphOnly
        )
        .unwrap());
    let graph_response = graph_context.commit().unwrap();
    assert_eq!(
        graph_response.property_value(CorePropertyTypeName::CommitRequestStatus).unwrap(),
        Some(BaseValue::StringValue("Complete".into())),
        "{:?}",
        graph_update.validation_findings().unwrap()
    );
    assert_eq!(graph_update.holon_id().unwrap(), updated.holon_id().unwrap());
    assert_eq!(
        graph_response.property_value(CorePropertyTypeName::DescriptorResetRequired).unwrap(),
        Some(BaseValue::BooleanValue(false.into()))
    );
    assert!(session_before_commit.check_active().is_ok());
    assert!(before_commit.value_kind().is_ok());
    assert_eq!(holon_type.effective_descriptor().unwrap().holon(), before_holon_commit.holon());

    // Schema version publication carries Extends/commitments and must retire both partitions.
    let schema_tx = runtime.session().begin_transaction().await.unwrap();
    let schema_context = runtime.session().get_transaction(&schema_tx).unwrap();
    let mut schema_update = schema_context
        .mutation()
        .stage_new_version_from_id(saved_value.holon_id().unwrap())
        .unwrap();
    schema_update.with_property_value("Description", "Descriptor retention reset proof").unwrap();
    let schema_response = schema_context.commit().unwrap();
    assert_eq!(
        schema_response.property_value(CorePropertyTypeName::CommitRequestStatus).unwrap(),
        Some(BaseValue::StringValue("Complete".into())),
        "{:?}",
        schema_update.validation_findings().unwrap()
    );
    assert_eq!(
        schema_response.property_value(CorePropertyTypeName::DescriptorResetRequired).unwrap(),
        Some(BaseValue::BooleanValue(true.into()))
    );
    assert!(matches!(before_commit.value_kind(), Err(HolonError::ObsoleteSession)));
    assert!(matches!(before_holon_commit.instance_properties(), Err(HolonError::ObsoleteSession)));
    assert!(
        registry.is_empty().unwrap(),
        "Commit response must not republish its received registry"
    );
    assert_eq!(value.value_kind().unwrap(), copied.value_kind().unwrap());
    assert!(!holon_type.effective_descriptor().unwrap().instance_properties().unwrap().is_empty());
    update_context.dispose().unwrap();
    graph_context.dispose().unwrap();
    schema_context.dispose().unwrap();
    snapshot.dispose().unwrap();
    context.dispose().unwrap();
    assert!(value.supported_operators().unwrap().len() >= 3);
    property.effective_constraints().unwrap();
}
