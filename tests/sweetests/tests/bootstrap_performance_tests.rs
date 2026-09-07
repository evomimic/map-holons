//! Focused Core Schema bootstrap measurement seam for Issue #688.
//!
//! Run with `npm run sweet:bootstrap-perf` and collect the aggregate `[PERF-688]`
//! lines. This test deliberately exercises the generated bootstrap bundle through
//! the same runtime path used by ordinary Sweettests, rather than a storage microbenchmark.

use holons_core::reference_layer::HolonSpaceBehavior;
use holons_test::{init_test_runtime, DancesTestCase};
use std::time::Instant;
use tracing::info;

#[tokio::test(flavor = "multi_thread")]
async fn measures_generated_core_schema_bootstrap() {
    let mut test_case = DancesTestCase::default();
    let started_at = Instant::now();
    let (runtime, transaction_id) = init_test_runtime(&mut test_case).await;

    assert!(
        runtime
            .session()
            .space_manager()
            .get_space_holon_id()
            .expect("Core Schema bootstrap space lookup must succeed")
            .is_some(),
        "focused measurement must bootstrap the CoreSchemaSpace anchor"
    );

    runtime
        .session()
        .archive_transaction(&transaction_id)
        .expect("focused measurement transaction must archive");

    info!(
        "[PERF-688] sweettest_bootstrap_measurement: total_ms={}",
        started_at.elapsed().as_millis(),
    );
}
