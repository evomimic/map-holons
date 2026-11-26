//! Schema fixtures and read-only startup discovery; neither authorizes acquisition nor supplies C4 rules.
use holons_core::RelationshipReadHint;
use holons_prelude::prelude::*;
use holons_test::DancesTestCase;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::sync::Arc;

const SCHEMA: &str = "MAP Agent Spaces Schema-v0.1.4";
const PERSON_TYPE: &str = "PersonAgent.BioSocialAgent";

async fn transaction(runtime: &Runtime) -> Arc<TransactionContext> {
    let id = runtime.session().begin_transaction().await.unwrap();
    runtime.session().get_transaction(&id).unwrap()
}

async fn execute(
    runtime: &Runtime,
    context: &Arc<TransactionContext>,
    action: TransactionAction,
) -> HolonReference {
    let result = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand { context: context.clone(), action }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap();
    let MapResult::Reference(response) = result else { panic!("expected reference: {result:?}") };
    response
}

async fn load(runtime: &Runtime, context: &Arc<TransactionContext>) -> HolonReference {
    execute(
        runtime,
        context,
        TransactionAction::LoadHolons {
            content_set: core_types::ContentSet {
                files_to_load: vec![core_types::FileData {
                    filename: "agent-spaces/schema.json".into(),
                    raw_contents: include_str!(
                        "../../../generated/json-imports/agent-spaces/schema.json"
                    )
                    .into(),
                }],
            },
        },
    )
    .await
}

fn saved(context: &Arc<TransactionContext>, key: &str) -> HolonReference {
    context.lookup().get_saved_holon_by_key(&key.into()).unwrap().into()
}

fn members(source: &HolonReference, name: &str) -> Vec<HolonReference> {
    // Verify persisted membership independently of intentionally long inverse cache lifetimes.
    source
        .related_holons_with_hint(name, RelationshipReadHint::RequireFresh)
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .clone()
}

fn status(response: &HolonReference, property: &str, expected: &str) {
    assert_eq!(
        response.property_value(property).unwrap(),
        Some(expected.to_base_value()),
        "{response:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_schema_preserves_space_and_name_versions_and_rejects_reimport() {
    let initiator = holons_test::harness::helpers::create_test_dance_initiator().await;
    let empty_space = holons_client::init_client_runtime(Some(initiator.clone()));
    let empty_context =
        empty_space.get_transaction_manager().open_public_transaction(empty_space.clone()).unwrap();
    // Infrastructure discovery must not create the legacy descriptor-less Space.
    assert!(empty_context.discover_local_holon_space().unwrap().is_none());
    assert!(empty_context.discover_local_holon_space().unwrap().is_none());
    assert!(empty_context.get_space_holon().unwrap().is_none());
    assert!(!empty_context.is_bootstrap_provisioning());
    let (runtime, initial) = holons_test::harness::helpers::init_runtime_with_initiator(
        &mut DancesTestCase::default(),
        initiator.clone(),
    )
    .await;
    let context = runtime.session().get_transaction(&initial).unwrap();
    let space = context.get_space_holon().unwrap().unwrap();
    let space_id = space.holon_id().unwrap();
    let space_key = space.key().unwrap().unwrap();
    let space_type = members(&space, "DescribedBy");
    let core_id = saved(&context, "MAP Core Schema-v0.0.12").holon_id().unwrap();
    // DAHN depends on PersonAgent, so bootstrap already supplies Agent Spaces.
    // Reopening must retain those definitions without another create import.
    let schema_id = saved(&context, SCHEMA).holon_id().unwrap();
    let person_type_id = saved(&context, PERSON_TYPE).holon_id().unwrap();
    runtime.session().archive_transaction(&initial).unwrap();

    // A fresh host proxy against the same conductor adopts committed identity/content
    // without loading Core or Agent Spaces again. This is not acquisition authorization.
    let reopened_space = holons_client::init_client_runtime(Some(initiator));
    let reopened = reopened_space
        .get_transaction_manager()
        .open_public_transaction(reopened_space.clone())
        .unwrap();
    assert!(reopened.get_space_holon().unwrap().is_none());
    let discovered = reopened.discover_local_holon_space().unwrap().unwrap();
    assert_eq!(discovered.holon_id().unwrap(), space_id);
    reopened.set_space_holon_id(discovered.holon_id().unwrap()).unwrap();
    assert_eq!(saved(&reopened, "MAP Core Schema-v0.0.12").holon_id().unwrap(), core_id);
    assert_eq!(saved(&reopened, SCHEMA).key().unwrap().unwrap().0, SCHEMA);
    assert_eq!(saved(&reopened, SCHEMA).holon_id().unwrap(), schema_id);
    assert_eq!(saved(&reopened, PERSON_TYPE).holon_id().unwrap(), person_type_id);
    assert!(!reopened.is_bootstrap_provisioning());

    let context = transaction(&runtime).await;
    let space = context.get_space_holon().unwrap().unwrap();
    assert_eq!(space.holon_id().unwrap(), space_id);
    assert_eq!(space.key().unwrap(), Some(space_key.clone()));
    assert_eq!(members(&space, "DescribedBy"), space_type);
    assert_eq!(saved(&context, "MAP Core Schema-v0.0.12").holon_id().unwrap(), core_id);
    assert!(members(&space, "IsInteriorOf").is_empty(), "import does not acquire the space");

    let person_type = saved(&context, PERSON_TYPE);
    let contract = person_type.holon_descriptor().unwrap();
    // Descriptor classification is independent of the inherited instance contract.
    assert_eq!(contract.header().type_name().unwrap().0, "MetaHolonType");
    let rule = saved(&context, "PersonAgentName.FormatRule");
    assert_eq!(rule.property_value("TemplateString").unwrap(), Some("{0}.[1].[2]".to_base_value()));
    assert_eq!(
        members(&rule, "TemplateParameters"),
        vec![
            saved(&context, "LastName.PropertyType"),
            saved(&context, "FirstName.PropertyType"),
            saved(&context, "MiddleName.PropertyType")
        ]
    );
    assert_eq!(saved(&context, SCHEMA).holon_id().unwrap(), schema_id);
    assert_eq!(person_type.holon_id().unwrap(), person_type_id);
    runtime.session().archive_transaction(&context.tx_id()).unwrap();

    let context = transaction(&runtime).await;
    let duplicate = load(&runtime, &context).await;
    status(&duplicate, "LoadCommitStatus", "Rejected");
    assert_eq!(
        duplicate.property_value("HolonsCommitted").unwrap(),
        Some(MapInteger(0).to_base_value())
    );
    assert!(context.staged_references().unwrap().iter().any(|c| c.validation_findings().unwrap().iter().any(|v| matches!(&v.kind, core_types::CommitValidationViolationKind::RuleViolation { code } if code == "DS-KEY-006"))));
    runtime.session().archive_transaction(&context.tx_id()).unwrap();

    let context = transaction(&runtime).await;
    assert_eq!(saved(&context, SCHEMA).holon_id().unwrap(), schema_id);
    assert_eq!(saved(&context, PERSON_TYPE).holon_id().unwrap(), person_type_id);
    let mut transient = context.mutation().new_holon(None).unwrap();
    transient.with_descriptor(saved(&context, PERSON_TYPE)).unwrap();
    let mut person = context.mutation().stage_new_holon(transient).unwrap();
    person
        .add_related_holons("HasInterior", vec![context.get_space_holon().unwrap().unwrap()])
        .unwrap();
    // Key and required name properties are independently validated; repair both before retry.
    let rejected = execute(&runtime, &context, TransactionAction::Commit).await;
    status(&rejected, "CommitRequestStatus", "Rejected");
    assert!(person.validation_findings().unwrap().iter().any(|v| matches!(&v.subject, core_types::ValidationSubjectPath::Property { name, .. } if name == "LastName")));
    person.with_property_value("LastName", "Smith").unwrap();
    person.with_property_value(CorePropertyTypeName::Key, "Smith").unwrap();
    let committed = execute(&runtime, &context, TransactionAction::Commit).await;
    status(&committed, "CommitRequestStatus", "Complete");
    let first_id = person.holon_id().unwrap();
    runtime.session().archive_transaction(&context.tx_id()).unwrap();

    let context = transaction(&runtime).await;
    let first: HolonReference =
        SmartReference::new_from_id(context.space_read_handle(), first_id.clone()).into();
    assert_eq!(first.key().unwrap(), Some("Smith".into()));
    assert_eq!(first.property_value("FirstName").unwrap(), None);
    assert_eq!(first.property_value("MiddleName").unwrap(), None);
    let interior = context.get_space_holon().unwrap().unwrap();
    assert_eq!(members(&first, "HasInterior"), vec![interior.clone()]);
    assert_eq!(members(&first, "OwnedBy"), vec![interior.clone()]);
    assert_eq!(members(&interior, "IsInteriorOf"), vec![first.clone()]);
    let inverse =
        interior.effective_descriptor().unwrap().get_relationship_by_name("IsInteriorOf").unwrap();
    assert_eq!(inverse.membership_cache_max_age_millis().unwrap(), 86_400_000);
    runtime.session().archive_transaction(&context.tx_id()).unwrap();

    let mut predecessor = first_id.clone();
    for (property, value) in [("FirstName", "Jane"), ("MiddleName", "Marie")] {
        let context = transaction(&runtime).await;
        let previous =
            SmartReference::new_from_id(context.space_read_handle(), predecessor.clone());
        let mut next = context.mutation().stage_new_version(previous).unwrap();
        next.with_property_value(property, value).unwrap();
        next.with_property_value(
            CorePropertyTypeName::Key,
            if property == "FirstName" { "Smith.Jane" } else { "Smith.Jane.Marie" },
        )
        .unwrap();

        status(
            &execute(&runtime, &context, TransactionAction::Commit).await,
            "CommitRequestStatus",
            "Complete",
        );
        let current_id = next.holon_id().unwrap();
        runtime.session().archive_transaction(&context.tx_id()).unwrap();

        let context = transaction(&runtime).await;
        let current: HolonReference =
            SmartReference::new_from_id(context.space_read_handle(), current_id).into();
        assert_eq!(
            current.key().unwrap(),
            Some(if property == "FirstName" { "Smith.Jane" } else { "Smith.Jane.Marie" }.into())
        );
        assert_eq!(current.predecessor().unwrap().unwrap().holon_id().unwrap(), predecessor);
        assert_eq!(members(&current, "HasInterior")[0].holon_id().unwrap(), space_id);
        let inverse = members(&context.get_space_holon().unwrap().unwrap(), "IsInteriorOf");
        assert_eq!(inverse.len(), 1, "versions contribute one sovereign lineage membership");
        assert_eq!(inverse[0].holon_id().unwrap(), first_id, "inverse is Lineage-bound");
        predecessor = current.holon_id().unwrap();
        runtime.session().archive_transaction(&context.tx_id()).unwrap();
    }
}

/// A partial/legacy provisioning footprint must not be mistaken for a fresh cell.
#[tokio::test(flavor = "multi_thread")]
async fn local_space_discovery_rejects_unindexed_content() {
    let backend = holons_test::harness::helpers::setup_test_conductor().await;
    let persisted: core_types::StoredHolonNode = backend
        .conductor
        .call(
            &backend.cell.zome("holons"),
            "holon_storage_persist",
            core_types::HolonWriteRequest::PublishRoot {
                holon_node: integrity_core_types::HolonNodeModel::new(
                    core_types::PropertyMap::from([(
                        core_types::PropertyName("title".into()),
                        "Interrupted provisioning".to_base_value(),
                    )]),
                ),
            },
        )
        .await;
    let transport = Arc::new(holons_trust_channel::TrustChannel::new(backend.clone()));
    let space = holons_client::init_client_runtime(Some(transport));
    let context = space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
    for _ in 0..2 {
        let error = context.discover_local_holon_space().unwrap_err();
        assert!(matches!(error, HolonError::InvalidState(_)), "{error:?}");
        assert!(context.get_space_holon().unwrap().is_none());
    }
    let original: Option<core_types::StoredHolonNode> = backend
        .conductor
        .call(
            &backend.cell.zome("holons"),
            "holon_storage_get",
            persisted.version_metadata.version_id.clone(),
        )
        .await;
    assert!(original.is_some());
}
