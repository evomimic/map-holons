use std::sync::Once;
use tracing::warn;
use tracing_subscriber::{filter::LevelFilter, fmt, fmt::format::FmtSpan, EnvFilter};

static INIT: Once = Once::new();
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::WARN;

pub fn init_tracing() {
    INIT.call_once(|| {
        // Try to use RUST_LOG, or fall back to the default level.
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            println!("RUST_LOG not set. Using default: {DEFAULT_LOG_LEVEL:?}");
            EnvFilter::default().add_directive(DEFAULT_LOG_LEVEL.into())
        });

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
