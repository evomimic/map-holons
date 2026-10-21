use std::sync::Once;
use tracing::warn;
use tracing_subscriber::{filter::LevelFilter, fmt, fmt::format::FmtSpan, EnvFilter};

static INIT: Once = Once::new();
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::WARN;

/// Sweettest conductors are isolated single-agent nodes, so every DHT publish fails with
/// `NoPeersForLocation` and is retried on each later commit, flooding WARN output. As in
/// Conductora's dev mode (`host/conductora/src/logging.rs`), these defaults go first so an
/// explicit `RUST_LOG` directive for the same target restores the warnings when needed.
const ISOLATED_CONDUCTOR_DEFAULTS: &str =
    "holochain::core::workflow::publish_dht_ops_workflow=error";

pub fn init_tracing() {
    INIT.call_once(|| {
        let filter = build_env_filter(std::env::var(EnvFilter::DEFAULT_ENV).ok().as_deref());

        let subscriber = fmt().with_env_filter(filter.clone()).with_target(true).with_test_writer();
        let emit_span_closes = std::env::var_os("MAP_HOLONS_TRACE_SPAN_CLOSES").is_some();

        // Existing Holochain workflow spans report busy and idle time on close.
        // Keep them opt-in because full Sweettest runs can create substantial output.
        let init_result = if emit_span_closes {
            subscriber.with_span_events(FmtSpan::CLOSE).try_init()
        } else {
            subscriber.try_init()
        };

        // Initialize tracing subscriber.
        match init_result {
            Ok(_) => {
                // Derive a readable level summary
                let level = filter.max_level_hint().unwrap_or(DEFAULT_LOG_LEVEL);

                warn!("✅ Tracing initialized at level: {level:?}");
            }
            Err(e) => {
                eprintln!("⚠️ Failed to initialize tracing subscriber: {e:?}");
            }
        }
    });
}

/// Use `RUST_LOG` when it parses, or fall back to the default level; both keep the
/// isolated-conductor defaults underneath.
fn build_env_filter(rust_log: Option<&str>) -> EnvFilter {
    rust_log
        .and_then(|rust_log| {
            EnvFilter::try_new(format!("{ISOLATED_CONDUCTOR_DEFAULTS},{rust_log}")).ok()
        })
        .unwrap_or_else(|| {
            println!("RUST_LOG not set. Using default: {DEFAULT_LOG_LEVEL:?}");
            EnvFilter::try_new(format!("{ISOLATED_CONDUCTOR_DEFAULTS},{DEFAULT_LOG_LEVEL}"))
                .expect("default tracing filter must parse")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::Level;
    use tracing_subscriber::prelude::*;

    const PUBLISH_TARGET: &str = "holochain::core::workflow::publish_dht_ops_workflow";

    fn enabled(rust_log: Option<&str>, target_is_publish: bool) -> bool {
        let subscriber = tracing_subscriber::registry().with(build_env_filter(rust_log));
        tracing::subscriber::with_default(subscriber, || {
            if target_is_publish {
                tracing::enabled!(target: PUBLISH_TARGET, Level::WARN)
            } else {
                tracing::enabled!(target: "holochain::conductor", Level::WARN)
            }
        })
    }

    #[test]
    fn publish_warnings_are_quiet_by_default_and_other_warnings_remain() {
        for rust_log in [None, Some("warn")] {
            assert!(!enabled(rust_log, true), "publish WARN enabled for RUST_LOG={rust_log:?}");
            assert!(enabled(rust_log, false), "other WARN disabled for RUST_LOG={rust_log:?}");
        }
    }

    #[test]
    fn explicit_rust_log_directive_restores_publish_warnings() {
        assert!(enabled(Some(&format!("warn,{PUBLISH_TARGET}=warn")), true));
    }
}
