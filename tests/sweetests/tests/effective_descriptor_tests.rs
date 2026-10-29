//! Guest preparation, Session reuse, and Commit-local shallow projection.
use holons_core::descriptors::{ComputedEffectiveSchema, Descriptor, EffectiveHolonDescriptor};
use holons_prelude::prelude::*;
use holons_test::{init_test_runtime, DancesTestCase};
use map_commands_contract::{
    HolonAction, HolonCommand, MapCommand, MapResult, ReadableHolonAction, SpaceCommand,
};
use map_commands_runtime::ExecutionPolicy;

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
    let descriptor = EffectiveHolonDescriptor::from_constructed_holon(references[0].clone());
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
    let direct = ComputedEffectiveSchema::resolve(target.holon()).unwrap();
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

    let snapshot =
        runtime.session().descriptor_registry().snapshot_into(&contexts[0]).unwrap().unwrap();
    assert_eq!(snapshot.tx_id(), contexts[0].tx_id());
    let copy =
        EffectiveHolonDescriptor::find_in_context(&contexts[0], target.holon()).unwrap().unwrap();
    assert_ne!(
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
    // Commit ships the shallow snapshot through the existing transient pool transport.
    let committed = contexts[1].commit().unwrap();
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
    assert!(descriptor.instance_properties().is_ok());

    let descriptor_owner = match &references[0] {
        HolonReference::Transient(reference) => reference.tx_id(),
        _ => panic!("transient descriptor"),
    };
    for context in contexts {
        runtime.session().dispose_transaction(&context.tx_id()).await.unwrap();
    }
    let owner_context = runtime.session().get_transaction(&descriptor_owner).unwrap();
    let read = HolonCommand {
        context: owner_context.clone(),
        target: holons_boundary::HolonReferenceWire::from(&references[0])
            .bind(&owner_context)
            .unwrap(),
        action: HolonAction::Read(ReadableHolonAction::GetInstanceProperties),
    };
    assert!(matches!(
        runtime.execute_command(MapCommand::Holon(read), ExecutionPolicy::default()).await.unwrap(),
        MapResult::Collection(_)
    ));
    assert!(!descriptor.instance_properties().unwrap().is_empty());
    eprintln!(
        "effective Holon descriptor: Session acquisition, reuse and snapshot completed in {:?}",
        started.elapsed()
    );
    drop(runtime);
    assert!(descriptor.instance_properties().is_err());
}
