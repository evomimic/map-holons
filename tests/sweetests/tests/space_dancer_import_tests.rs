//! Fresh-state regression coverage for schema-driven Dancer discovery.
//!
//! Conductora imports a Dancer package only after CoreSchemaSpace bootstrap.
//! The package's Dancer-side relationship must therefore be committed before a
//! later transaction can discover it through the local space's inverse edge.

use base_types::MapString;
use core_types::{ContentSet, FileData};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::reference_layer::{HolonReference, ReadableHolon};
use holons_test::{init_test_runtime, DancesTestCase};
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::sync::Arc;
use type_names::DancerRelationshipTypeName;

async fn begin_transaction(runtime: &Runtime) -> Arc<TransactionContext> {
    let result = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .expect("beginning transaction must succeed");
    let MapResult::TransactionCreated { tx_id } = result else {
        panic!("expected TransactionCreated, got {result:?}");
    };
    runtime.session().get_transaction(&tx_id).expect("transaction must be available")
}

fn space_navigator_content_set() -> ContentSet {
    let repository_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = repository_root.join("generated/house-troupe/space-navigator/imports/schema.json");
    let raw_contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    ContentSet {
        files_to_load: vec![FileData { filename: path.display().to_string(), raw_contents }],
    }
}

async fn load_space_navigator(runtime: &Runtime) {
    let context = begin_transaction(runtime).await;
    let result = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context,
                action: TransactionAction::LoadHolons {
                    content_set: space_navigator_content_set(),
                },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .expect("Space Navigator import must succeed");
    assert!(
        matches!(result, MapResult::Reference(HolonReference::Transient(_))),
        "Space Navigator import must return its load response"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn importing_space_navigator_affords_its_dancer_from_the_local_space() {
    let mut test_case = DancesTestCase::default();
    let (runtime, initial_tx_id) = init_test_runtime(&mut test_case).await;
    runtime
        .session()
        .archive_transaction(&initial_tx_id)
        .expect("initial transaction must archive");

    load_space_navigator(&runtime).await;

    let context = begin_transaction(&runtime).await;
    let local_space = context
        .get_space_holon()
        .expect("reading local HolonSpace must succeed")
        .expect("Core bootstrap must provide a local HolonSpace");
    let dancer = HolonReference::Smart(
        context
            .lookup()
            .get_saved_holon_by_key(&MapString::from("SpaceNavigator.Dancer"))
            .expect("Space Navigator Dancer must be saved"),
    );

    let forward = dancer
        .related_holons(DancerRelationshipTypeName::AffordedByHolonSpace)
        .expect("Dancer forward relationship must resolve")
        .read()
        .expect("Dancer forward relationship must be readable")
        .get_members()
        .clone();
    assert!(
        forward.iter().any(|member| member.holon_id() == local_space.holon_id()),
        "SpaceNavigator.Dancer must target the active local HolonSpace"
    );

    let afforded_dancers = local_space
        .related_holons(DancerRelationshipTypeName::AffordsDancer)
        .expect("HolonSpace inverse relationship must resolve")
        .read()
        .expect("HolonSpace inverse relationship must be readable")
        .get_members()
        .clone();
    assert!(
        afforded_dancers.iter().any(|member| member.holon_id() == dancer.holon_id()),
        "local HolonSpace must afford SpaceNavigator.Dancer after package import"
    );
}
