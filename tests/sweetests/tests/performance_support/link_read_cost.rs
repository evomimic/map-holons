//! Read-only control through Holochain 0.7's public persisted-store API.
//! The former authored/DHT/cache SQL controls do not exist in this storage architecture.
use holochain::prelude::{ActionHash, LinkTag, LinkType, LinkTypeFilter, ZomeIndex};
use holochain_state::{dht_store::DhtStoreRead, query::link::GetLinksFilter};
use serde_json::{json, Value};
use std::time::Instant;

pub async fn measure(
    store: DhtStoreRead,
    base: ActionHash,
    prefix: Vec<u8>,
    samples: u32,
) -> Value {
    let base = base.into();
    let types = LinkTypeFilter::single_type(ZomeIndex(0), LinkType(1));
    let tag = LinkTag(prefix);
    let filter = GetLinksFilter::default();
    let mut rows = Vec::new();
    for sample in 0..=samples {
        let started = Instant::now();
        let links = store.get_links(&base, &types, Some(&tag), &filter).await.unwrap();
        let elapsed_us = started.elapsed().as_micros() as u64;
        assert!(links.is_empty(), "empty persisted query must return no links");
        rows.push(json!({"sample":sample,"operation":"empty_persisted_links","total_us":elapsed_us,"rows":links.len()}));
    }
    json!({"api":"DhtStore::get_links","database":"per_dna_dht","rows":rows})
}
