use std::sync::Once;
use tracing_subscriber::{fmt, EnvFilter};

static INIT: Once = Once::new();

/// Initializes the tracing subscriber for tests.
/// Will only run once per test binary, even if called in multiple test functions.
///
/// Logging is controlled via the RUST_LOG env var, e.g.
///     RUST_LOG=debug cargo test
pub fn init_tracing() {
    INIT.call_once(|| {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        fmt().with_env_filter(filter).with_target(true).with_test_writer().init();
    });
}
