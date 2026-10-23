//! Matched guest-only HDK link timings against a freshly bootstrapped disposable conductor.
use holons_test::{init_runtime_with_initiator, setup_probe_enabled_conductor, DancesTestCase};
use holons_trust_channel::TrustChannel;
use integrity_core_types::RelationshipName;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "explicit populated-store HDK performance capture"]
async fn compare_default_and_local_link_strategy() {
    let output = std::env::var("MAP_LINK_STRATEGY_OUTPUT").expect("evidence output path required");
    let samples: u32 = 200;
    holons_test::init_tracing();
    let backend = setup_probe_enabled_conductor().await;
    let (runtime, tx_id) = init_runtime_with_initiator(
        &mut DancesTestCase::default(),
        Arc::new(TrustChannel::new(backend.clone())),
    )
    .await;
    let context = runtime.session().get_transaction(&tx_id).unwrap();
    let mut cases = Vec::new();
    for (key, relationship, count) in [
        ("HolonSpace.HolonType", "DescribedBy", 1),
        ("HolonSpace.HolonType", "Extends", 1),
        ("StringValue.ValueVisualizer", "UsedByVisualizerUsage", 0),
    ] {
        let source = context.lookup().get_saved_holon_by_key(&key.into()).unwrap();
        let id = source.holon_id().local_id().clone();
        let started = Instant::now();
        let (rows, links): (Vec<(u32, String, u64, usize)>, Vec<String>) = backend
            .conductor
            .call(
                &backend.cell.zome("holons_test_probes"),
                "smartlink_measure_get_strategy_for_test",
                (id, RelationshipName(relationship.into()), samples),
            )
            .await;
        let roundtrip_us = started.elapsed().as_micros();
        assert_eq!(links.len(), count, "unexpected bucket population for {key}/{relationship}");
        assert_eq!(rows.len(), (3 * samples + 2) as usize);
        assert!(rows.iter().all(|row| row.3 == count));
        eprintln!(
            "[LINK-STRATEGY] {key}/{relationship} rows={} links={} roundtrip_us={roundtrip_us}",
            rows.len(),
            links.len()
        );
        cases.push(json!({"source":key,"relationship":relationship,"links":links,
            "samples_per_strategy":samples,"roundtrip_us":roundtrip_us,
            "columns":["sample","strategy","hdk_us","returned_links"],"rows":rows}));
    }
    let source =
        context.lookup().get_saved_holon_by_key(&"StringValue.ValueVisualizer".into()).unwrap();
    let host_controls: Vec<(u32, String, u64)> = backend
        .conductor
        .call(
            &backend.cell.zome("holons_test_probes"),
            "smartlink_measure_host_baseline_for_test",
            (
                source.holon_id().local_id().clone(),
                RelationshipName("UsedByVisualizerUsage".into()),
                samples,
            ),
        )
        .await;
    assert_eq!(host_controls.len(), 4 * (samples + 1) as usize);
    std::fs::write(&output,serde_json::to_vec_pretty(&json!({"cases":cases,
        "host_control_columns":["sample","operation","us"],"host_controls":host_controls,
        "timing":"guest sys_time immediately around HDK get_links; sample 0 retained separately and excluded from repeated statistics",
        "order":"default/local alternates pair order; local_repeated follows; four host controls rotate order",
        "population":"normal full Core Schema bootstrap; fresh single-agent local rendezvous conductor; full storage authority; publishing/gossip disabled; no benchmark writes"})).unwrap()).unwrap();
    runtime.session().archive_transaction(&tx_id).unwrap();
    drop(source);
    drop(context);
    drop(runtime);
    backend.shutdown().await;
    eprintln!("[LINK-STRATEGY] evidence={output}");
}
