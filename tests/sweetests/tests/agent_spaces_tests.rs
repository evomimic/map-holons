//! Schema-level sovereignty fixtures; these do not authorize acquisition or supply missing C4 rules.
use holons_core::RelationshipReadHint;
use holons_prelude::prelude::*;
use holons_test::{harness::helpers::init_test_runtime, DancesTestCase};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::sync::Arc;

const SCHEMA: &str = "MAP Agent Spaces Schema-v0.1.0";
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
    let (runtime, initial) = init_test_runtime(&mut DancesTestCase::default()).await;
    let context = runtime.session().get_transaction(&initial).unwrap();
    let space = context.get_space_holon().unwrap().unwrap();
    let space_id = space.holon_id().unwrap();
    let space_key = space.key().unwrap().unwrap();
    let space_type = members(&space, "DescribedBy");
    let core_id = saved(&context, "MAP Core Schema-v0.0.7").holon_id().unwrap();
    assert!(context.lookup().get_saved_holon_by_key(&SCHEMA.into()).is_err());

    let response = load(&runtime, &context).await;
    status(&response, "LoadCommitStatus", "Complete");
    runtime.session().archive_transaction(&initial).unwrap();

    let context = transaction(&runtime).await;
    let space = context.get_space_holon().unwrap().unwrap();
    assert_eq!(space.holon_id().unwrap(), space_id);
    assert_eq!(space.key().unwrap(), Some(space_key.clone()));
    assert_eq!(members(&space, "DescribedBy"), space_type);
    assert_eq!(saved(&context, "MAP Core Schema-v0.0.7").holon_id().unwrap(), core_id);
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
    let schema_id = saved(&context, SCHEMA).holon_id().unwrap();
    let person_type_id = person_type.holon_id().unwrap();
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
    let mut transient = context.mutation().new_holon(Some("Smith..".into())).unwrap();
    transient.with_descriptor(saved(&context, PERSON_TYPE)).unwrap();
    let mut person = context.mutation().stage_new_holon(transient).unwrap();
    person
        .add_related_holons("HasInterior", vec![context.get_space_holon().unwrap().unwrap()])
        .unwrap();
    // The explicit key is fixture input, not evidence of FormatRule execution.
    let rejected = execute(&runtime, &context, TransactionAction::Commit).await;
    status(&rejected, "CommitRequestStatus", "Rejected");
    assert!(person.validation_findings().unwrap().iter().any(|v| matches!(&v.subject, core_types::ValidationSubjectPath::Property { name, .. } if name == "LastName")));
    person.with_property_value("LastName", "Smith").unwrap();
    let committed = execute(&runtime, &context, TransactionAction::Commit).await;
    status(&committed, "CommitRequestStatus", "Complete");
    runtime.session().archive_transaction(&context.tx_id()).unwrap();

    let context = transaction(&runtime).await;
    let first = saved(&context, "Smith..");
    let first_id = first.holon_id().unwrap();
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

    let mut previous_key = "Smith..";
    let mut predecessor = first_id.clone();
    for (property, value, key) in
        [("FirstName", "Jane", "Smith.Jane."), ("MiddleName", "Marie", "Smith.Jane.Marie")]
    {
        let context = transaction(&runtime).await;
        let previous = context.lookup().get_saved_holon_by_key(&previous_key.into()).unwrap();
        let mut next = context.mutation().stage_new_version(previous).unwrap();
        next.with_property_value(property, value).unwrap();
        next.with_property_value("Key", key).unwrap();
        status(
            &execute(&runtime, &context, TransactionAction::Commit).await,
            "CommitRequestStatus",
            "Complete",
        );
        runtime.session().archive_transaction(&context.tx_id()).unwrap();

        let context = transaction(&runtime).await;
        let current = saved(&context, key);
        assert_eq!(current.predecessor().unwrap().unwrap().holon_id().unwrap(), predecessor);
        assert_eq!(members(&current, "HasInterior")[0].holon_id().unwrap(), space_id);
        let inverse = members(&context.get_space_holon().unwrap().unwrap(), "IsInteriorOf");
        assert_eq!(inverse.len(), 1, "versions contribute one sovereign lineage membership");
        assert_eq!(inverse[0].holon_id().unwrap(), first_id, "inverse is Lineage-bound");
        predecessor = current.holon_id().unwrap();
        previous_key = key;
        runtime.session().archive_transaction(&context.tx_id()).unwrap();
    }
}
