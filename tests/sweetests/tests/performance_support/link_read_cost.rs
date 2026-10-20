//! Native controls on the same disposable stores; no schema or index mutations.
use holochain::prelude::{ActionHash, LinkTag, LinkType, LinkTypeFilter, ZomeIndex};
use holochain_sqlite::{
    db::{DbKindT, DbWrite},
    error::DatabaseError,
};
use holochain_state::query::{
    link::{GetLinksFilter, GetLinksQuery},
    Query,
};
use serde_json::{json, Value};
use std::time::Instant;

pub async fn measure<K: DbKindT>(
    db: DbWrite<K>,
    label: &'static str,
    base: ActionHash,
    prefix: Vec<u8>,
    samples: u32,
) -> Value {
    let population = db
        .read_async::<DatabaseError, _, _>(|txn| {
            let counts: (i64, i64) = txn.query_row(
                "SELECT (SELECT COUNT(*) FROM Action), (SELECT COUNT(*) FROM DhtOp)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            Ok(counts)
        })
        .await
        .unwrap();
    let query = GetLinksQuery::new(
        base.into(),
        LinkTypeFilter::single_type(ZomeIndex(0), LinkType(1)),
        Some(LinkTag(prefix)),
        GetLinksFilter::default(),
    );
    let mut rows = Vec::new();
    for sample in 0..=samples {
        for offset in 0..2 {
            let links = (sample + offset) % 2 == 1;
            let started = Instant::now();
            let mut conn = db.get_read_txn().await.unwrap();
            let acquire_us = started.elapsed().as_micros() as u64;
            let queued = Instant::now();
            let query = query.clone();
            let row = tokio::task::spawn_blocking(move || -> Result<Value, DatabaseError> {
                let queue_us = queued.elapsed().as_micros() as u64;
                let began = Instant::now();
                let txn = conn.transaction()?;
                let transaction_us = began.elapsed().as_micros() as u64;
                let sql = if links {query.query()} else {"SELECT 1".into()};
                let params = query.params();
                let began = Instant::now();
                let mut statement = txn.prepare(&sql)?;
                let prepare_us = began.elapsed().as_micros() as u64;
                let began = Instant::now();
                let count = if links {
                    let mut rows = statement.query(params.as_slice())?;
                    let mut count=0;
                    while rows.next()?.is_some() {count+=1;}
                    assert_eq!(count,0,"empty native query must return no links");
                    count
                } else {
                    assert_eq!(statement.query_row([], |row| row.get::<_,i64>(0))?,1);
                    1
                };
                let execute_us = began.elapsed().as_micros() as u64;
                drop(statement);
                let began = Instant::now();
                drop(txn);
                drop(conn);
                let release_us = began.elapsed().as_micros() as u64;
                Ok(json!({"sample":sample,"operation":if links {"empty_link_sql"} else {"select_1"},"acquire_us":acquire_us,"blocking_queue_us":queue_us,"transaction_begin_us":transaction_us,"prepare_us":prepare_us,"execute_us":execute_us,"release_us":release_us,"rows":count}))
            }).await.unwrap().unwrap();
            let mut row = row;
            row["total_us"] = json!(started.elapsed().as_micros() as u64);
            rows.push(row);
        }
    }
    json!({"database":label,"action_rows":population.0,"dht_op_rows":population.1,"sql":query.query(),"rows":rows})
}
