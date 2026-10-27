//! Guest construction and ordinary transaction projection of derived schema graphs.
use holons_core::descriptors::{Descriptor, EffectiveDescriptor, HolonDescriptor};
use holons_prelude::prelude::*;
use holons_test::{init_test_runtime, DancesTestCase};
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::ExecutionPolicy;

#[tokio::test(flavor = "multi_thread")]
async fn guest_construction_returns_a_retained_sealed_schema_graph() {
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
    let target: HolonReference = context
        .lookup()
        .get_saved_holon_by_key(&MapString("HolonSpace.HolonType".into()))
        .unwrap()
        .into();
    let runtime_schema: HolonReference = context
        .lookup()
        .get_saved_holon_by_key(&MapString("MAP Descriptor Runtime Schema-v0.1.0".into()))
        .unwrap()
        .into();
    for key in [
        "EffectiveDescriptor.HolonType",
        "EffectiveSchemaContribution.HolonType",
        "ConstructEffectiveDescriptor.DanceType",
        "ConstructEffectiveDescriptorRequest.HolonType",
        "ConstructEffectiveDescriptorResponse.DanceResponseType",
    ] {
        let definition: HolonReference =
            context.lookup().get_saved_holon_by_key(&MapString(key.into())).unwrap().into();
        let owners = definition.related_holons("ComponentOf").unwrap();
        let owners = owners.read().unwrap();
        assert_eq!(owners.get_members().len(), 1);
        assert!(holons_core::descriptors::same_definition(
            &owners.get_members()[0],
            &runtime_schema
        )
        .unwrap());
    }
    let origins = holons_core::descriptors::ancestors(&target).unwrap();
    let authoritative = [
        ("InstanceProperties", "(HolonType.TypeDescriptor)-[InstanceProperties]->(PropertyType.TypeDescriptor)"),
        ("InstanceRelationships", "(HolonType.TypeDescriptor)-[InstanceRelationships]->(DeclaredRelationshipType.RelationshipType)"),
        ("AffordsDance", "(HolonType.TypeDescriptor)-[AffordsDance]->(DanceType.HolonType)"),
        ("AffordsCommand", "(HolonType.TypeDescriptor)-[AffordsCommand]->(CommandType.HolonType)"),
        ("ValidationBindings", "(TypeDescriptor)-[ValidationBindings]->(ValidationRule.HolonType)"),
        ("Constraints", "(TypeDescriptor)-[Constraints]->(Constraint.HolonType)"),
        ("InstanceKeyRule", "(HolonType.TypeDescriptor)-[InstanceKeyRule]->(KeyRuleType.HolonType)"),
    ].into_iter().map(|(name, key)| {
        let reference: HolonReference = context.lookup().get_saved_holon_by_key(&MapString(key.into())).unwrap().into();
        (name, reference)
    }).collect::<Vec<_>>();
    let direct = HolonDescriptor::from_holon(target.clone());
    let mut previous = None;
    for _ in 0..2 {
        let MapResult::Reference(reference) = runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context: context.clone(),
                    action: TransactionAction::ConstructEffectiveDescriptor {
                        target: target.clone(),
                        include_contributions: true,
                    },
                }),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap()
        else {
            panic!("expected result reference")
        };
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
        assert!(holons_core::descriptors::same_definition(
            result.described_holon_type().unwrap().holon(),
            &target
        )
        .unwrap());
        let properties = |properties: Vec<holons_core::descriptors::PropertyDescriptor>| {
            properties.into_iter().map(|p| p.holon().full_reference_id_string()).collect::<Vec<_>>()
        };
        assert_eq!(
            properties(result.instance_properties().unwrap()),
            properties(direct.instance_properties().unwrap())
        );
        let dances = |dances: Vec<holons_core::descriptors::DanceDescriptor>| {
            dances.into_iter().map(|d| d.holon().full_reference_id_string()).collect::<Vec<_>>()
        };
        assert_eq!(
            dances(result.afforded_dances().unwrap()),
            dances(direct.afforded_dances().unwrap())
        );
        // The default validator still rejects the known descriptor-category
        // mismatch; only the dedicated conformance helper accommodates it.
        let strict =
            holons_validation::validate_transient_contract(&context, &reference).unwrap_err();
        assert!(strict.to_string().contains("incompatible describing type"), "{strict}");
        // Inspect provenance and exercise exhaustive output conformance below;
        // production construction no longer runs that additional validation pass.
        let contributions = result.contributions().unwrap();
        assert!(!contributions.is_empty());
        let mut collection_names = std::collections::HashSet::new();
        let mut positions = std::collections::HashMap::<String, Vec<i64>>::new();
        for contribution in &contributions {
            assert!(contribution.holon().key().unwrap().is_none());
            let owners = contribution.holon().related_holons("OwnedBy").unwrap();
            let owners = owners.read().unwrap();
            assert_eq!(owners.get_members().len(), 1);
            assert!(holons_core::descriptors::same_definition(&owners.get_members()[0], &owner)
                .unwrap());
            let collection = contribution.holon().related_holons("ContributionCollection").unwrap();
            let collection = collection.read().unwrap();
            assert_eq!(collection.get_members().len(), 1);
            let (name, expected) = authoritative
                .iter()
                .find(|(_, expected)| {
                    holons_core::descriptors::same_definition(
                        &collection.get_members()[0],
                        expected,
                    )
                    .unwrap()
                })
                .expect("only the seven authoritative collections produce provenance");
            if collection_names.insert(*name) {
                // Exercise typed declared/inverse access once per collection identity.
                assert!(holons_core::descriptors::same_definition(
                    contribution.collection().unwrap().holon(),
                    expected
                )
                .unwrap());
            }
            positions.entry((*name).into()).or_default().push(contribution.position().unwrap());
            let member = contribution.member().unwrap();
            assert!(origins.iter().any(|origin| holons_core::descriptors::same_definition(
                origin,
                &member.declared_on
            )
            .unwrap()));
            assert!(!member.member.full_reference_id_string().is_empty());
        }
        for sequence in positions.values_mut() {
            sequence.sort();
            assert_eq!(*sequence, (0..sequence.len() as i64).collect::<Vec<_>>());
        }
        assert!(collection_names.contains("AffordsDance"));
        assert!(collection_names.contains("AffordsCommand"));
        // Generated output conformance is verified here instead of on every runtime call.
        holons_validation::validate_effective_descriptor_dance_contract(&context, &reference)
            .unwrap();
        for contribution in &contributions {
            holons_validation::validate_effective_descriptor_dance_contract(
                &context,
                contribution.holon(),
            )
            .unwrap();
        }
        // A real RelationshipType outside the seven canonical identities cannot
        // masquerade as an authoritative collection, even at this boundary.
        let original = &contributions[0];
        let mut malformed = context.mutation().new_holon(None).unwrap();
        malformed.with_descriptor(original.holon().get_descriptor().unwrap().unwrap()).unwrap();
        malformed.with_property_value("ContributionPosition", 0_i64).unwrap();
        let unrelated: HolonReference = context
            .lookup()
            .get_saved_holon_by_key(&MapString(
                "(RelationshipType.TypeDescriptor)-[SourceType]->(HolonType.TypeDescriptor)".into(),
            ))
            .unwrap()
            .into();
        malformed.add_related_holons("ContributionCollection", vec![unrelated]).unwrap();
        let member = original.member().unwrap();
        malformed.add_related_holons("ContributionMember", vec![member.member]).unwrap();
        malformed.add_related_holons("ContributionDeclaredOn", vec![member.declared_on]).unwrap();
        let error = holons_validation::validate_effective_descriptor_dance_contract(
            &context,
            &malformed.into(),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("Not an authoritative contribution collection"),
            "{error}"
        );
        let mut sealed = reference.clone();
        assert!(sealed.with_property_value("Corrupt", true).is_err());
        if let Some(previous) = previous {
            assert!(!holons_core::descriptors::same_definition(&reference, &previous).unwrap());
        }
        previous = Some(reference);
    }
    let retained = EffectiveDescriptor::try_from_holon(previous.unwrap()).unwrap();
    context.dispose().unwrap();
    assert!(retained.instance_properties().is_err());
}
