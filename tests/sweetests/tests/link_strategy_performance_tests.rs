//! Explicit strategy comparison against the full bootstrap population, never an ordinary suite.
use holons_test::harness::helpers::init_probe_test_runtime;
use holons_test::DancesTestCase;
use integrity_core_types::RelationshipName;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "explicit populated-store HDK performance experiment"]
async fn compare_default_and_local_link_strategy() {
    let output = std::env::var("MAP_LINK_STRATEGY_OUTPUT").expect("evidence output path required");
    let samples: u32 = std::env::var("MAP_LINK_STRATEGY_SAMPLES")
        .unwrap_or_else(|_| "200".into())
        .parse()
        .unwrap();
    let (runtime, tx_id, control) = init_probe_test_runtime(&mut DancesTestCase::default()).await;
    let context = runtime.session().get_transaction(&tx_id).unwrap();
    let mut cases = Vec::new();
    for (key, relationship, populated) in [
        ("HolonSpace.HolonType", "DescribedBy", true),
        ("HolonSpace.HolonType", "Extends", true),
        ("StringValue.ValueVisualizer", "UsedByVisualizerUsage", false),
    ] {
        let source = context.lookup().get_saved_holon_by_key(&key.into()).unwrap();
        let id = source.holon_id().local_id().clone();
        let started = std::time::Instant::now();
        let (rows, links) =
            control.measure_get_strategy(id, RelationshipName(relationship.into()), samples).await;
        assert_eq!(
            !links.is_empty(),
            populated,
            "unexpected bucket population for {key}/{relationship}"
        );
        assert_eq!(rows.len(), (3 * samples + 2) as usize);
        eprintln!(
            "[LINK-STRATEGY] {key}/{relationship} rows={} links={} roundtrip_ms={}",
            rows.len(),
            links.len(),
            started.elapsed().as_millis()
        );
        cases.push(json!({"source":key,"relationship":relationship,"links":links,
            "samples_per_strategy":samples,"roundtrip_ms":started.elapsed().as_millis(),
            "columns":["sample","strategy","hdk_us","returned_links"],"rows":rows}));
    }
    std::fs::write(&output,serde_json::to_vec_pretty(&json!({"cases":cases,
        "timing":"guest sys_time immediately around HDK get_links; sample 0 is first pair, excluded from repeated statistics",
        "order":"default/local alternates pair order each sample; local_repeated follows alternating phase",
        "population":"normal full Core Schema bootstrap; single-agent local-only SweetConductor; no benchmark writes"})).unwrap()).unwrap();
    eprintln!("[LINK-STRATEGY] evidence={output}");
}

#[path = "performance_support/link_read_cost.rs"]
mod link_read_cost;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "explicit populated-store host-call and database cost experiment"]
async fn separate_host_call_and_empty_query_costs() {
    use holons_test::harness::helpers::init_probe_performance_runtime;
    let output = std::env::var("MAP_LINK_COST_OUTPUT").expect("evidence path required");
    let samples = 200;
    let (runtime, tx_id, backend) =
        init_probe_performance_runtime(&mut DancesTestCase::default()).await;
    let context = runtime.session().get_transaction(&tx_id).unwrap();
    let source =
        context.lookup().get_saved_holon_by_key(&"StringValue.ValueVisualizer".into()).unwrap();
    let source_id = source.holon_id();
    let relationship = RelationshipName("UsedByVisualizerUsage".into());
    let guest: Vec<(u32, String, u64)> = backend
        .conductor
        .call(
            &backend.cell.zome("holons_test_probes"),
            "smartlink_measure_host_baseline_for_test",
            (source_id.local_id().clone(), relationship.clone(), samples),
        )
        .await;
    eprintln!("[LINK-COST] guest comparison complete; {} observations", guest.len());
    let prefix = core_types::smartlink_relationship_prefix(&relationship).unwrap();
    let base = holochain::prelude::ActionHash::from_raw_39(source_id.local_id().0.clone());
    let store =
        link_read_cost::measure(backend.cell.dht_store().as_read(), base, prefix, samples).await;
    eprintln!("[LINK-COST] persisted-store control complete");
    std::fs::write(output,serde_json::to_vec_pretty(&json!({"guest_columns":["sample","operation","us"],"guest":guest,"persisted_store":store,"notes":"Guest timings preserve the 0.6 experiment. The 0.7 native control measures the public DhtStore::get_links API, not the removed 0.6 authored/DHT/cache SQL phases; these native measurements are not directly comparable. Sample 0 is excluded from statistics."})).unwrap()).unwrap();
}
