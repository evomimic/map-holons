//! Descriptor attachment and the final loader pass over saved and staged contracts.
mod execution_steps;
mod fixture_cases;

use fixture_cases::load_holons_internal_fixture::{
    add_loader_relationship_reference, make_load_set_from_bundles, BundleWithFilename,
};
use holons_prelude::prelude::*;
use holons_test::{assert_descriptor_completion, init_test_runtime, DancesTestCase};
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::{collections::BTreeSet, sync::Arc};

async fn begin(runtime: &Runtime) -> Arc<TransactionContext> {
    let result = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap();
    let MapResult::TransactionCreated { tx_id } = result else { panic!("expected transaction") };
    runtime.session().get_transaction(&tx_id).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn attachment_defaults_persist_through_public_commit() {
    execution_steps::dance_test_runner::run_dance_test_suite(
        execution_steps::dance_test_runner::DanceTestSuite {
            name: "attachment_defaults",
            test_cases: vec![
                fixture_cases::attachment_defaults_fixture::attachment_defaults_fixture().unwrap(),
            ],
        },
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn mixed_import_promotes_saved_descriptor_and_materializes_early_enum_defaults() {
    let (runtime, initial_tx) = init_test_runtime(&mut DancesTestCase::default()).await;
    runtime.session().archive_transaction(&initial_tx).unwrap();
    let context = begin(&runtime).await;
    // This contract is saved before the extension is assembled. Its existing enum
    // member remains visible when Pass 2a promotes it and reattaches its meta-type.
    let document = serde_json::json!({"holons": [{
        "key": "MixedDefaults.HolonType", "type": "MetaHolonType.MetaTypeDescriptor",
        "properties": {"TypeName": "MixedDefaults", "TypeNamePlural": "MixedDefaultsTypes",
            "DisplayName": "Mixed defaults", "DisplayNamePlural": "Mixed default types",
            "Description": "Saved contract for extension import"},
        "relationships": [
            {"name": "Extends", "target": [{"$ref": "HolonType.TypeDescriptor"}]},
            {"name": "ComponentOf", "target": [{"$ref": "MAP Core Schema-v0.0.7"}]},
            {"name": "InstanceProperties", "target": [{"$ref": "TargetBinding.PropertyType"}]}
        ]
    }]});
    let result = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: context.clone(),
                action: TransactionAction::LoadHolons {
                    content_set: core_types::ContentSet {
                        files_to_load: vec![core_types::FileData {
                            filename: "mixed-defaults-base.json".into(),
                            raw_contents: document.to_string(),
                        }],
                    },
                },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap();
    let MapResult::Reference(response) = result else { panic!("expected loader response") };
    assert_eq!(
        response.property_value("LoadCommitStatus").unwrap(),
        Some("Complete".to_base_value())
    );
    assert_descriptor_completion(&context, BTreeSet::from(["MixedDefaults.HolonType".into()]));

    let context = begin(&runtime).await;
    let original = context
        .lookup()
        .get_saved_holon_by_key(&MapString("MixedDefaults.HolonType".into()))
        .unwrap();
    let original_id = original.holon_id();
    let original_properties = original.into_model().unwrap().property_map;
    let mut property =
        context.mutation().new_holon(Some(MapString("ExtraChoice.PropertyType".into()))).unwrap();
    for (name, value) in [
        ("TypeName", "ExtraChoice"),
        ("TypeNamePlural", "ExtraChoices"),
        ("DisplayName", "Extra choice"),
        ("DisplayNamePlural", "Extra choices"),
        ("Description", "Enum added to the saved contract"),
        ("DefaultValue", "Lineage"),
    ] {
        property.with_property_value(name, value).unwrap();
    }
    property.with_property_value("IsValueRequired", true).unwrap();
    for (name, source, target) in [
        ("DescribedBy", "ExtraChoice.PropertyType", "MetaPropertyType.MetaTypeDescriptor"),
        // These references intentionally address the saved source without importing
        // another LoaderHolon for it, exercising resolve_staged_write_source.
        ("DescribedBy", "MixedDefaults.HolonType", "MetaHolonType.MetaTypeDescriptor"),
        ("Extends", "ExtraChoice.PropertyType", "PropertyType.TypeDescriptor"),
        ("ComponentOf", "ExtraChoice.PropertyType", "MAP Core Schema-v0.0.7"),
        ("ValueType", "ExtraChoice.PropertyType", "TargetBinding.MapEnumValueType"),
        ("InstanceProperties", "MixedDefaults.HolonType", "ExtraChoice.PropertyType"),
    ] {
        add_loader_relationship_reference(&context, &mut property, name, source, &[target])
            .unwrap();
    }
    let mut subject =
        context.mutation().new_holon(Some(MapString("MixedDefaults.Instance".into()))).unwrap();
    add_loader_relationship_reference(
        &context,
        &mut subject,
        "DescribedBy",
        "MixedDefaults.Instance",
        &["MixedDefaults.HolonType"],
    )
    .unwrap();
    let mut bundle = context
        .mutation()
        .new_holon(Some(MapString("MixedDefaults.ExtensionBundle".into())))
        .unwrap();
    bundle
        .add_related_holons(
            CoreRelationshipTypeName::BundleMembers,
            vec![property.into(), subject.into()],
        )
        .unwrap();
    let load_set = make_load_set_from_bundles(
        &context,
        "MixedDefaults.ExtensionSet",
        vec![BundleWithFilename::new(bundle, "mixed-defaults-extension.json")],
    )
    .unwrap();
    let response = context.load_holons_and_commit(load_set).unwrap();

    eprintln!("Loader response: {:?}", response.into_model().unwrap().property_map);

    let errors = response.related_holons(CoreRelationshipTypeName::HasLoadError).unwrap();
    let members = errors.read().unwrap().get_members().clone();
    for error in members {
        eprintln!("Loader error: {:?}", error.into_model().unwrap().property_map);
    }

    assert_eq!(
        response.property_value("LoadCommitStatus").unwrap(),
        Some("Complete".to_base_value())
    );
    assert_eq!(response.property_value("ErrorCount").unwrap(), Some(0_i64.to_base_value()));
    assert_eq!(
        response.property_value("ValidationViolationCount").unwrap(),
        Some(0_i64.to_base_value())
    );
    assert_eq!(response.property_value("HolonsCommitted").unwrap(), Some(3_i64.to_base_value()));
    assert_descriptor_completion(
        &context,
        BTreeSet::from(["MixedDefaults.HolonType".into(), "ExtraChoice.PropertyType".into()]),
    );
    let staged = context.staged_references().unwrap();
    let replacement = staged
        .iter()
        .find(|s| s.key().unwrap() == Some(MapString("MixedDefaults.HolonType".into())))
        .unwrap();
    assert_ne!(replacement.holon_id().unwrap(), original_id);
    assert_eq!(replacement.predecessor().unwrap().unwrap().holon_id().unwrap(), original_id);
    let members = replacement
        .related_holons("InstanceProperties")
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .clone();
    let member_keys: BTreeSet<_> = members.iter().map(|p| p.key().unwrap().unwrap().0).collect();
    assert_eq!(
        member_keys,
        BTreeSet::from(["TargetBinding.PropertyType".into(), "ExtraChoice.PropertyType".into()])
    );
    let instance = staged
        .iter()
        .find(|s| s.key().unwrap() == Some(MapString("MixedDefaults.Instance".into())))
        .unwrap();
    for (name, token) in [("TargetBinding", "Version"), ("ExtraChoice", "Lineage")] {
        assert_eq!(
            instance.property_value(name).unwrap(),
            Some(MapEnumValue(MapString(token.into())).to_base_value())
        );
    }
    assert_eq!(original.into_model().unwrap().property_map, original_properties);
    assert_eq!(
        original.related_holons("InstanceProperties").unwrap().read().unwrap().get_members().len(),
        1
    );
    // Read the saved instance in a new observer to prove native enum kinds persisted.
    let observer = begin(&runtime).await;
    let saved = observer
        .lookup()
        .get_saved_holon_by_key(&MapString("MixedDefaults.Instance".into()))
        .unwrap();
    for (name, token) in [("TargetBinding", "Version"), ("ExtraChoice", "Lineage")] {
        assert_eq!(
            saved.property_value(name).unwrap(),
            Some(MapEnumValue(MapString(token.into())).to_base_value())
        );
    }
}
