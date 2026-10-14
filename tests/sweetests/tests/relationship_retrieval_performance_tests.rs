//! Controlled storage benchmark; no runtime membership cache or production heuristic.
use base_types::MapString;
use core_types::{HolonWriteRequest, SmartLink, StoredHolonNode};
use holons_test::harness::helpers::setup_probe_enabled_conductor;
use integrity_core_types::{HolonNodeModel, LocalId, PropertyMap, RelationshipName};
use serde_json::json;
use std::time::Instant;

const STRATEGIES: [&str; 3] = ["separate_prefixes", "batched_prefixes", "consolidated"];
fn name(index: usize) -> RelationshipName {
    RelationshipName(MapString(format!("BenchmarkRelationship{index:02}")))
}
async fn endpoint(backend: &holons_test::MockConductorConfig) -> LocalId {
    let stored: StoredHolonNode = backend
        .conductor
        .call(
            &backend.cell.zome("holons"),
            "holon_storage_persist",
            HolonWriteRequest::PublishRoot { holon_node: HolonNodeModel::new(PropertyMap::new()) },
        )
        .await;
    stored.version_id().clone()
}
fn canonical(buckets: &[Vec<SmartLink>]) -> Vec<Vec<String>> {
    buckets
        .iter()
        .map(|bucket| {
            let mut members: Vec<_> =
                bucket.iter().map(|link| serde_json::to_string(link).unwrap()).collect();
            members.sort();
            members
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "explicit performance capture; run sweet:relationship-perf"]
async fn compare_relationship_retrieval_strategies() {
    let samples: usize = std::env::var("MAP_RELATIONSHIP_PERF_SAMPLES")
        .unwrap_or_else(|_| "6".into())
        .parse()
        .unwrap();
    assert!(samples >= 3 && samples <= 60, "use 3..60 samples");
    let output =
        std::env::var("MAP_RELATIONSHIP_PERF_OUTPUT").expect("capture output path required");
    let backend = setup_probe_enabled_conductor().await;
    let probe = backend.cell.zome("holons_test_probes");
    let target = endpoint(&backend).await;
    let mut rows = Vec::new();
    // Even distribution isolates source-size and demand effects; skew models sparse demand.
    for (total, skewed) in
        [(0usize, false), (16, false), (128, false), (129, false), (512, false), (512, true)]
    {
        let source = endpoint(&backend).await;
        let counts: Vec<u32> = (0..16)
            .map(|index| {
                if skewed {
                    if index == 0 {
                        497
                    } else {
                        1
                    }
                } else {
                    (total / 16 + usize::from(index < total % 16)) as u32
                }
            })
            .collect();
        let distribution: Vec<_> =
            counts.iter().enumerate().map(|(i, count)| (name(i), *count)).collect();
        let seeded: u32 = backend
            .conductor
            .call(
                &probe,
                "smartlink_seed_retrieval_for_test",
                (source.clone(), target.clone(), distribution),
            )
            .await;
        assert_eq!(seeded as usize, total);
        for requested in [1usize, 2, 4, 8, 16] {
            // Include both high-selectivity and sparse/empty queries on the same source.
            for start in [0usize, 8, 16] {
                let names: Vec<_> = (start..start + requested).map(name).collect();
                let expected: Vec<usize> = (start..start + requested)
                    .map(|i| counts.get(i).copied().unwrap_or(0) as usize)
                    .collect();
                let mut reference = None;
                // One untimed warm-up per strategy; rotate subsequent timed call order.
                for round in 0..=samples {
                    for offset in 0..3 {
                        let strategy = STRATEGIES[(round + offset) % 3];
                        let started = Instant::now();
                        let (guest_us, calls, queries, returned, buckets): (
                            u64,
                            u64,
                            u64,
                            u64,
                            Vec<Vec<SmartLink>>,
                        ) = backend
                            .conductor
                            .call(
                                &probe,
                                "smartlink_measure_retrieval_for_test",
                                (source.clone(), names.clone(), strategy.to_string()),
                            )
                            .await;
                        let roundtrip_us = started.elapsed().as_micros() as u64;
                        assert_eq!(buckets.iter().map(Vec::len).collect::<Vec<_>>(), expected);
                        let actual = canonical(&buckets);
                        if let Some(reference) = &reference {
                            assert_eq!(&actual, reference);
                        } else {
                            reference = Some(actual);
                        }
                        assert_eq!(
                            calls,
                            if strategy == "separate_prefixes" { requested as u64 } else { 1 }
                        );
                        assert_eq!(
                            queries,
                            if strategy == "consolidated" { 1 } else { requested as u64 }
                        );
                        let selected: usize = expected.iter().sum();
                        assert_eq!(
                            returned as usize,
                            if strategy == "consolidated" { total } else { selected }
                        );
                        if round > 0 {
                            rows.push(json!({"source_links": total, "distribution": if skewed {"skewed"} else {"uniform"},
                                "requested": requested, "start": start, "selected_links": selected,
                                "strategy": strategy, "sample": round, "position": offset,
                                "guest_us": guest_us, "roundtrip_us": roundtrip_us,
                                "backend_calls": calls, "queries": queries, "returned_links": returned}));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(rows.len(), 6 * 5 * 3 * samples * 3);
    std::fs::write(&output, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
    println!("relationship retrieval capture: {} samples written to {output}", rows.len());
}
