//! Guest construction and ordinary transaction projection of a retained schema result.
use holons_core::descriptors::{Descriptor, EffectiveDescriptor, HolonDescriptor};
use holons_prelude::prelude::*;
use holons_test::{init_test_runtime, DancesTestCase};
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::ExecutionPolicy;
use std::collections::BTreeSet;

fn identities(references: impl IntoIterator<Item = HolonReference>) -> BTreeSet<String> {
    references.into_iter().map(|reference| reference.full_reference_id_string()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn guest_construction_returns_a_retained_sealed_schema_graph() {
    let started = std::time::Instant::now();
    let (runtime, _) = init_test_runtime(&mut DancesTestCase::default()).await;
    eprintln!("effective descriptor: bootstrap finished at {:?}", started.elapsed());
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
    let target: HolonReference = context
        .lookup()
        .get_saved_holon_by_key(&MapString("HolonSpace.HolonType".into()))
        .unwrap()
        .into();
    let direct = HolonDescriptor::from_holon(target.clone());
    let mut previous = None;
    for iteration in 0..2 {
        let before = context.lookup().transient_count().unwrap();
        let MapResult::Reference(reference) = runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context: context.clone(),
                    action: TransactionAction::ConstructEffectiveDescriptor {
                        target: target.clone(),
                    },
                }),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap()
        else {
            panic!("expected result reference")
        };
        eprintln!(
            "effective descriptor: construction {iteration} finished at {:?}",
            started.elapsed()
        );
        // One result plus ordinary request, invocation and response Holons.
        assert_eq!(context.lookup().transient_count().unwrap() - before, 4);
        assert!(reference.is_transient());
        assert!(reference.key().unwrap().is_none());
        let owner = context.get_space_holon().unwrap().unwrap();
        let owners = reference.related_holons("OwnedBy").unwrap();
        let owners = owners.read().unwrap();
        assert_eq!(owners.get_members().len(), 1);
        assert!(
            holons_core::descriptors::same_definition(&owners.get_members()[0], &owner).unwrap()
        );
        drop(owners);
        let result = EffectiveDescriptor::try_from_holon(reference.clone()).unwrap();
        assert!(result.contributions().unwrap().is_empty());
        assert!(holons_core::descriptors::same_definition(
            result.described_holon_type().unwrap().holon(),
            &target,
        )
        .unwrap());
        assert_eq!(
            identities(
                result
                    .instance_properties()
                    .unwrap()
                    .into_iter()
                    .map(|entry| entry.holon().clone())
            ),
            identities(
                direct
                    .instance_properties()
                    .unwrap()
                    .into_iter()
                    .map(|entry| entry.holon().clone())
            ),
        );
        assert_eq!(
            identities(
                result
                    .instance_relationships()
                    .unwrap()
                    .into_iter()
                    .map(|entry| entry.holon().clone())
            ),
            identities(
                direct
                    .effective_declared_relationships()
                    .unwrap()
                    .into_iter()
                    .map(|entry| entry.holon().clone())
            ),
        );
        assert_eq!(
            identities(
                result.afforded_dances().unwrap().into_iter().map(|entry| entry.holon().clone())
            ),
            identities(
                direct.afforded_dances().unwrap().into_iter().map(|entry| entry.holon().clone())
            ),
        );
        assert_eq!(
            identities(
                result.afforded_commands().unwrap().into_iter().map(|entry| entry.holon().clone())
            ),
            identities(
                direct.afforded_commands().unwrap().into_iter().map(|entry| entry.holon().clone())
            ),
        );
        assert_eq!(
            identities(result.effective_constraints().unwrap()),
            identities(
                direct.effective_constraints().unwrap().into_iter().map(|entry| entry.member)
            ),
        );
        assert_eq!(
            identities(result.effective_validation_bindings().unwrap()),
            identities(
                direct
                    .effective_validation_bindings()
                    .unwrap()
                    .into_iter()
                    .map(|entry| entry.member)
            ),
        );
        assert!(holons_core::descriptors::same_definition(
            result.effective_key_rule().unwrap().holon(),
            direct.effective_key_rule().unwrap().holon(),
        )
        .unwrap());
        let navigable = reference.related_holons("NavigableRelationships").unwrap();
        let navigable = navigable.read().unwrap();
        assert!(!navigable.get_members().is_empty());
        assert_eq!(
            navigable.get_members().len(),
            identities(navigable.get_members().clone()).len()
        );
        drop(navigable);
        let mut sealed = reference.clone();
        assert!(sealed.with_property_value("Corrupt", true).is_err());
        if let Some(previous) = previous {
            assert!(!holons_core::descriptors::same_definition(&reference, &previous).unwrap());
        }
        previous = Some(reference);
        eprintln!(
            "effective descriptor: iteration {iteration} finished at {:?}",
            started.elapsed()
        );
    }
    let retained = EffectiveDescriptor::try_from_holon(previous.unwrap()).unwrap();
    context.dispose().unwrap();
    assert!(retained.instance_properties().is_err());
}
