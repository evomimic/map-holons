//! Composition of saved visualizers stops at the Visualizer family root.
use holons_prelude::prelude::*;
use map_commands_contract::{MapCommand, MapResult, SpaceCommand};
use map_commands_runtime::{ExecutionPolicy, Runtime};

pub async fn saved_visualizer_composition_stops_before_generic_descriptor_ancestors(
    runtime: &Runtime,
) {
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
    // Real type-owned composition must finish at the Visualizer family root;
    // its generic descriptor parents have no HasSlot policy to resolve.
    for (key, expected_slots) in
        [("HolonInspector.NodeVisualizer", 8), ("PathInspector.RootedNavigationVisualizer", 1)]
    {
        let visualizer: HolonReference =
            context.lookup().get_saved_holon_by_key(&MapString(key.into())).unwrap().into();
        assert_eq!(
            holons_core::dahn::composition_slots(&visualizer).unwrap().len(),
            expected_slots
        );
    }
    context.dispose().unwrap();
}
