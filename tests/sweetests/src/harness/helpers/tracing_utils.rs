use std::sync::Once;
use tracing::warn;
use tracing_subscriber::{filter::LevelFilter, fmt, EnvFilter};


static INIT: Once = Once::new();
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::WARN;

pub fn init_tracing() {
    INIT.call_once(|| {
        // Try to use RUST_LOG, or fall back to the default level.
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            println!("RUST_LOG not set. Using default: {DEFAULT_LOG_LEVEL:?}");
            EnvFilter::default().add_directive(DEFAULT_LOG_LEVEL.into())
        });

        // Initialize tracing subscriber.
        match fmt().with_env_filter(filter.clone()).with_target(true).with_test_writer().try_init()
        {
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
