//! Read-only timings of Holochain's persisted link-query API on a disposable conductor.
//!
//! These measure pool acquisition, SQL reads, decoding, Rust-side filtering, and result ordering.
//! They exclude WASM dispatch, MAP reference caches, source discovery, and result assertions.
//! Holochain already indexes Link(base_hash); this is not an index experiment or a comparison
//! with the former authored/DHT/cache database timings.
use holochain::prelude::{
    ActionData, AnyLinkableHash, Link, LinkTag, LinkType, LinkTypeFilter, ZomeIndex,
};
use holochain_state::dht_store::DhtStoreRead;
use holochain_state::query::link::GetLinksFilter;
use holons_test::MockConductorConfig;
use integrity_core_types::RelationshipName;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Instant;

const SAMPLES: usize = 5;
// Matches the sole integrity zome's LinkTypes::SmartLink discriminant.
const SMARTLINK_ZOME: ZomeIndex = ZomeIndex(0);
const SMARTLINK_TYPE: LinkType = LinkType(1);

fn query_shapes() -> Vec<(&'static str, Option<LinkTag>)> {
    let owns = RelationshipName(base_types::MapString::from("Owns"));
    let successor = RelationshipName(base_types::MapString::from("Successor"));
    vec![
        ("SingleScan", None),
        (
            "OwnsExactKeyPrefix",
            Some(LinkTag(
                core_types::smartlink_exact_key_prefix(
                    &owns,
                    &core_types::CanonicalKey::new("TypeDescriptor").unwrap(),
                )
                .unwrap(),
            )),
        ),
        ("OwnsPrefix", Some(LinkTag(core_types::smartlink_relationship_prefix(&owns).unwrap()))),
        (
            "SuccessorPrefix",
            Some(LinkTag(core_types::smartlink_relationship_prefix(&successor).unwrap())),
        ),
    ]
}

/// Select the busiest authored base and, when distinct, one with 4–16 creates. Counts include
/// subsequently deleted links; live result counts are measured separately through get_links.
fn select_sources(counts: BTreeMap<AnyLinkableHash, usize>) -> Vec<(AnyLinkableHash, usize)> {
    let mut sources: Vec<_> = counts.into_iter().collect();
    sources.sort_by(|(a, ac), (b, bc)| bc.cmp(ac).then_with(|| a.cmp(b)));
    let mut selected = Vec::new();
    if let Some(source) = sources.first() {
        selected.push(source.clone());
    }
    if let Some(source) = sources.iter().find(|(_, count)| (4..=16).contains(count)) {
        if !selected.iter().any(|(base, _)| base == &source.0) {
            selected.push(source.clone());
        }
    }
    selected
}

async fn read_links(
    store: &DhtStoreRead,
    base: &AnyLinkableHash,
    tag: Option<&LinkTag>,
) -> Vec<Link> {
    store
        .get_links(
            base,
            &LinkTypeFilter::single_type(SMARTLINK_ZOME, SMARTLINK_TYPE),
            tag,
            &GetLinksFilter::default(),
        )
        .await
        .expect("persisted SmartLink query must succeed")
}

/// Compare every filtered query with the full live-link baseline, including all link fields and
/// multiplicity. Sort only outside the timed region because equal-timestamp ordering can vary.
async fn observe(store: &DhtStoreRead, base: &AnyLinkableHash) -> Value {
    let mut baseline = read_links(store, base, None).await;
    baseline.sort();
    let shapes = query_shapes();
    let expected: Vec<Vec<Link>> = shapes
        .iter()
        .map(|(_, prefix)| {
            baseline
                .iter()
                .filter(|link| prefix.as_ref().is_none_or(|p| link.tag.0.starts_with(&p.0)))
                .cloned()
                .collect()
        })
        .collect();
    let mut samples = vec![Vec::new(); shapes.len()];
    // One warm-up per shape, then rotate query order to distribute cache/order effects.
    for round in 0..=SAMPLES {
        for offset in 0..shapes.len() {
            let index = (round + offset) % shapes.len();
            let (shape, prefix) = &shapes[index];
            let started = Instant::now();
            let mut links = read_links(store, base, prefix.as_ref()).await;
            let elapsed_us = started.elapsed().as_micros() as u64;
            links.sort();
            assert_eq!(links, expected[index], "{shape} must match the live-link baseline");
            if round > 0 {
                samples[index]
                    .push(json!({"round": round, "position": offset, "elapsed_us": elapsed_us}));
            }
        }
    }
    let mut after = read_links(store, base, None).await;
    after.sort();
    assert_eq!(baseline, after, "live links changed during profiling; discard this capture");
    json!({
        "base": base.to_string(),
        "live_smartlinks": baseline.len(),
        "observations": shapes.iter().enumerate().map(|(i, (shape, prefix))| json!({
            "shape": shape,
            "tag_prefix_bytes": prefix.as_ref().map(|tag| &tag.0),
            "returned_links": expected[i].len(),
            "samples": samples[i],
        })).collect::<Vec<_>>()
    })
}

/// Capture the current per-DNA store after Commit timing has finished. Source discovery uses
/// only this isolated agent's committed SmartLink creates; no database schema is changed.
pub async fn profile(backend: &MockConductorConfig, output: &str) {
    let store = backend.cell.dht_store().as_read();
    let records = store
        .source_chain_records(backend.cell.agent_pubkey(), false, true)
        .await
        .expect("source discovery must read the committed source chain");
    let mut counts = BTreeMap::new();
    for record in &records {
        if let ActionData::CreateLink(link) = &record.action().data {
            if link.zome_index == SMARTLINK_ZOME && link.link_type == SMARTLINK_TYPE {
                *counts.entry(link.base_address.clone()).or_insert(0) += 1;
            }
        }
    }
    let sources = select_sources(counts);
    assert!(!sources.is_empty(), "link-query profiling requires committed SmartLink creates");
    let mut observations = Vec::new();
    for (base, count) in sources {
        let mut observation = observe(&store, &base).await;
        observation["authored_create_links"] = json!(count);
        observations.push(observation);
    }
    let evidence = json!({
        "schema_version": 2,
        "measurement": "persisted_link_query",
        "api": "DhtStore::get_links",
        "database": "per_dna_dht",
        "phase": "after_commit_samples",
        "schema_modified": false,
        "warmup_rounds": 1,
        "samples_per_shape": SAMPLES,
        "source_selection": "largest authored SmartLink create count and distinct 4..16-create base",
        "authored_actions": records.len(),
        "sources": observations,
    });
    std::fs::write(output, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
    println!("Link-query timings: evidence={output}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain::prelude::{
        Action, ActionHash, ActionHeader, AgentPubKey, ChainOpType, CreateLinkData, DeleteLinkData,
        DnaHash, EntryHash, RenderedOp, RenderedOps, Signature, Timestamp,
    };
    use holochain_data::kind::Dht;
    use holochain_state::dht_store::DhtStore;
    use std::sync::Arc;

    fn base(seed: u8) -> AnyLinkableHash {
        EntryHash::from_raw_36(vec![seed; 36]).into()
    }

    #[test]
    fn source_selection_is_deterministic_and_does_not_duplicate_the_largest_base() {
        assert!(select_sources(BTreeMap::new()).is_empty());
        assert_eq!(select_sources(BTreeMap::from([(base(1), 8)])), vec![(base(1), 8)]);
        let sources = BTreeMap::from([(base(3), 8), (base(2), 8), (base(1), 100), (base(4), 2)]);
        assert_eq!(select_sources(sources), vec![(base(1), 100), (base(2), 8)]);
    }

    /// Seed through the supported store cache API so the test exercises the real schema,
    /// link tombstones, and prefix filtering without a WASM build or a running conductor.
    async fn cache_action(store: &DhtStore, sequence: u32, data: ActionData) -> ActionHash {
        let op_type = match &data {
            ActionData::CreateLink(_) => ChainOpType::CreateLink,
            ActionData::DeleteLink(_) => ChainOpType::DeleteLink,
            _ => unreachable!(),
        };
        let action = Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1; 36]),
                timestamp: Timestamp::from_micros(sequence as i64),
                action_seq: sequence,
                prev_action: Some(ActionHash::from_raw_36(vec![2; 36])),
            },
            data,
        };
        let op = RenderedOp::new(action, Signature::from([0; 64]), None, op_type).unwrap();
        let hash = op.action.hashed.hash.clone();
        store
            .cache_chain_ops(&RenderedOps { entry: None, ops: vec![op], warrant: None })
            .await
            .unwrap();
        hash
    }

    #[tokio::test]
    async fn timings_preserve_live_link_multiplicity_and_filter_prefixes() {
        let store = DhtStore::new_test(Dht::new(Arc::new(DnaHash::from_raw_36(vec![0; 36]))))
            .await
            .unwrap();
        let shapes = query_shapes();
        let owns_tag = shapes[1].1.clone().unwrap();
        let successor_tag = shapes[3].1.clone().unwrap();
        let create = |base_address, tag, link_type| {
            ActionData::CreateLink(CreateLinkData {
                base_address,
                target_address: base(10),
                zome_index: SMARTLINK_ZOME,
                link_type,
                tag,
            })
        };
        // Two identical targets/tags with distinct create actions must remain two results.
        cache_action(&store, 1, create(base(1), owns_tag.clone(), SMARTLINK_TYPE)).await;
        cache_action(&store, 2, create(base(1), owns_tag.clone(), SMARTLINK_TYPE)).await;
        cache_action(&store, 3, create(base(1), successor_tag, SMARTLINK_TYPE)).await;
        let deleted =
            cache_action(&store, 4, create(base(1), owns_tag.clone(), SMARTLINK_TYPE)).await;
        cache_action(&store, 5, create(base(1), owns_tag.clone(), LinkType(0))).await;
        cache_action(&store, 6, create(base(2), owns_tag, SMARTLINK_TYPE)).await;
        cache_action(
            &store,
            7,
            ActionData::DeleteLink(DeleteLinkData {
                base_address: base(1),
                link_add_address: deleted,
            }),
        )
        .await;

        let evidence = observe(&store.as_read(), &base(1)).await;
        assert_eq!(evidence["live_smartlinks"], 3);
        for (observation, count) in
            evidence["observations"].as_array().unwrap().iter().zip([3, 2, 2, 1])
        {
            assert_eq!(observation["returned_links"], count);
            assert_eq!(observation["samples"].as_array().unwrap().len(), SAMPLES);
        }
        let empty = observe(&store.as_read(), &base(99)).await;
        assert_eq!(empty["live_smartlinks"], 0);
        for observation in empty["observations"].as_array().unwrap() {
            assert_eq!(observation["returned_links"], 0);
        }
    }
}
