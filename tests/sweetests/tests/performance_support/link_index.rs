//! Explicit, disposable-conductor schema experiment. Never called by ordinary suites.
use holochain::prelude::{ActionHash, LinkTag, LinkType, LinkTypeFilter, ZomeIndex};
use holochain_sqlite::db::{DbKindT, DbWrite};
use holochain_sqlite::error::DatabaseError;
use holochain_sqlite::rusqlite::Transaction;
use holochain_state::query::link::{GetLinksFilter, GetLinksQuery};
use holochain_state::query::Query;
use holons_test::MockConductorConfig;
use integrity_core_types::RelationshipName;
use serde_json::{json, Value};
use std::time::Instant;

fn observe(
    txn: &Transaction<'_>,
    base: &[u8],
    prefix: Option<Vec<u8>>,
) -> Result<(Value, Vec<Vec<u8>>), DatabaseError> {
    let query = GetLinksQuery::new(
        ActionHash::from_raw_39(base.to_vec()).into(),
        LinkTypeFilter::single_type(ZomeIndex(0), LinkType(1)),
        prefix.map(LinkTag),
        GetLinksFilter::default(),
    );
    let sql = query.query();
    let params = query.params();
    let plan = txn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?
        .query_map(params.as_slice(), |row| row.get::<_, String>(3))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut micros = Vec::new();
    let mut result = Vec::new();
    for iteration in 0..6 {
        let started = Instant::now();
        let mut rows = txn
            .prepare(&sql)?
            .query_map(params.as_slice(), |row| row.get::<_, Vec<u8>>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let elapsed = started.elapsed().as_micros() as u64;
        rows.sort();
        if iteration > 0 {
            assert_eq!(result, rows, "query observation must remain stable within the transaction");
            micros.push(elapsed);
        }
        result = rows;
    }
    Ok((json!({"sql":sql,"plan":plan,"samples_us":micros,"action_rows":result.len()}), result))
}

async fn database<K: DbKindT>(db: DbWrite<K>, label: &'static str, indexed: bool) -> Value {
    db.write_async::<DatabaseError, _, _>(move |txn| {
        let population: (i64,i64) = txn.query_row(
            "SELECT (SELECT COUNT(*) FROM Action), (SELECT COUNT(*) FROM DhtOp)", [],
            |row| Ok((row.get(0)?,row.get(1)?)),
        )?;
        let index_before: i64 = txn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='Action_base_hash_perf_idx'",[],|row|row.get(0))?;
        assert_eq!(index_before,0,"the conductor must begin without the experimental index");
        let sources = txn.prepare("SELECT base_hash,COUNT(*) AS n FROM Action WHERE link_type=1 AND zome_index=0 AND base_hash IS NOT NULL GROUP BY base_hash ORDER BY n DESC")?
            .query_map([],|row| Ok((row.get::<_,Vec<u8>>(0)?,row.get::<_,i64>(1)?)))?
            .collect::<Result<Vec<_>,_>>()?;
        let mut selected = Vec::new();
        if let Some(source)=sources.first(){selected.push(source.clone());}
        if let Some(source)=sources.iter().find(|(_,count)|*count>=4 && *count<=16){
            if !selected.iter().any(|(base,_)|base==&source.0){selected.push(source.clone());}
        }
        let prefixes = [("SingleScan",None),
            ("OwnsExactKeyPrefix",Some(core_types::smartlink_exact_key_prefix(&RelationshipName(base_types::MapString::from("Owns")), &core_types::CanonicalKey::new("TypeDescriptor").unwrap()).unwrap())),
            ("OwnsPrefix",Some(core_types::smartlink_relationship_prefix(&RelationshipName(base_types::MapString::from("Owns"))).unwrap())),
            ("SuccessorPrefix",Some(core_types::smartlink_relationship_prefix(&RelationshipName(base_types::MapString::from("Successor"))).unwrap()))];
        let mut before = Vec::new();
        for (base,count) in &selected {
            for (shape,prefix) in &prefixes {
                let (observation,rows)=observe(txn,base,prefix.clone())?;
                before.push((base.clone(),*count,*shape,prefix.clone(),observation,rows));
            }
        }
        let started = Instant::now();
        if indexed { txn.execute_batch("CREATE INDEX Action_base_hash_perf_idx ON Action(base_hash)")?; }
        let build_us = started.elapsed().as_micros() as u64;
        let mut observations=Vec::new();
        for (base,count,shape,prefix,observation,rows) in before {
            let (after,after_rows)=observe(txn,&base,prefix)?;
            assert_eq!(rows,after_rows,"index creation must not change exact query results");
            observations.push(json!({"source_action_rows":count,"shape":shape,"before":observation,"after":after}));
        }
        Ok(json!({"database":label,"action_rows":population.0,"operation_rows":population.1,
            "indexed":indexed,"index_build_us":build_us,"observations":observations}))
    }).await.expect("isolated database experiment must succeed")
}

pub async fn profile(backend: &MockConductorConfig, indexed: bool, output: &str) {
    let authored = database(backend.cell.authored_db().clone(), "authored", indexed).await;
    let dht = database(backend.cell.dht_db().clone(), "dht", indexed).await;
    let cache_db =
        backend.conductor.raw_handle().get_cache_db(backend.cell.cell_id()).await.unwrap();
    let cache = database(cache_db, "cache", indexed).await;
    std::fs::write(
        output,
        serde_json::to_vec_pretty(&json!({"indexed":indexed,"databases":[authored,dht,cache]}))
            .unwrap(),
    )
    .unwrap();
    println!("Link-index experiment: indexed={indexed}, evidence={output}");
}
