//! Logging initialisation for Conductora.
//!
//! ## Usage
//!
//! Call [`init_logging`] once at the top of `main()` before anything else.
//!
//! ## Environment variables
//!
//! ### `RUST_LOG` — host-side tracing filter
//!
//! Controls what the Rust tracing subscriber on the **host** emits.
//! Supports the shorthands below as well as standard `crate=level` directives.
//!
//! | Shorthand    | What it enables                                                         |
//! |--------------|-------------------------------------------------------------------------|
//! | *(not set)*  | Identical to `host:warn`                                                |
//! | `host:debug` | All host-side app crates at DEBUG + holochain noise suppressed          |
//! | `host:info`  | All host-side app crates at INFO  + holochain noise suppressed          |
//! | `host:warn`  | All host-side app crates at WARN  + holochain noise suppressed          |
//! | `host:error` | All host-side app crates at ERROR + holochain noise suppressed          |
//! | `debug`      | Literally everything at DEBUG (very verbose)                            |
//! | `warn`       | Literally everything at WARN                                            |
//! | `error`      | Literally everything at ERROR                                           |
//!
//! ### `WASM_LOG` — wasm/zome log passthrough
//!
//! Holochain's conductor reads `WASM_LOG` to decide which zome log events to
//! forward to the host tracing layer.  This module also reads it so the same
//! level is applied to the wasm targets in the host tracing filter — both must
//! be set for zome logs to appear.
//!
//! ```sh
//! WASM_LOG=debug RUST_LOG=host:debug cargo tauri dev
//! ```
//!
//! | `WASM_LOG=` | What you see                                              |
//! |-------------|-----------------------------------------------------------|
//! | *(not set)* | No zome logs forwarded (default)                          |
//! | `debug`     | All zome tracing events at DEBUG and above                |
//! | `info`      | Zome INFO, WARN, ERROR events                             |
//! | `warn`      | Zome WARN and ERROR events only                           |
//! | `error`     | Zome ERROR events only                                    |

use std::str::FromStr;
use tracing_subscriber::EnvFilter;

// ---------------------------------------------------------------------------
// Crate groups
// ---------------------------------------------------------------------------

/// Host-side application crates owned by this project.
const HOST_CRATES: &[&str] = &[
    "conductora_lib",     // Tauri application logic (setup, commands, config)
    "holons_client",      // Client-side holons state management
    "holons_receptor",    // Receptor trait implementations
    "holochain_receptor", // Holochain-specific receptor
    "holons_recovery",    // SQLite transaction recovery store
];

/// Tracing targets injected by holochain when a zome calls `hdk::tracing!`.
/// These must also be allowed by the host tracing filter or events are dropped.
/// The conductor only forwards these events when `WASM_LOG` is set.
const WASM_TARGETS: &[&str] = &[
    "wasm",             // Generic wasm log target emitted by holochain
    "holons_guest",     // Coordinator zome crate name
    "holons_integrity", // Integrity zome crate name
];

/// Holochain ecosystem crates whose log output is suppressed at most levels.
/// Raise individual entries here if you need to debug the network/DHT layer.
const HOLOCHAIN_NOISY_CRATES: &[(&str, &str)] = &[
    ("holochain", "warn"),         // Core conductor — very chatty at info/debug
    ("holochain_sqlite", "error"), // DB layer — migration messages etc.
    ("holochain_types", "warn"),   // Type-system internals
    ("holochain_p2p", "warn"),     // P2P networking
    ("kitsune2_core", "error"),    // Kitsune2 DHT core
    ("kitsune2_gossip", "error"),  // Gossip protocol
    ("kitsune2_dht", "error"),     // DHT internals
    ("tx5", "warn"),               // WebRTC / signal transport
    ("sbd_client", "warn"),        // SBD signal client
    ("holochain_runtime", "info"), // Tauri plugin runtime wrapper
    ("lair_keystore", "warn"),     // Lair key-management daemon
    ("tracing", "warn"),           // tracing framework internals
];

// ---------------------------------------------------------------------------
// Public entry-point
// ---------------------------------------------------------------------------

/// Initialise the global tracing subscriber.
///
/// Reads `RUST_LOG` for host-side filter shorthands and `WASM_LOG` for the
/// zome log level.  Must be called exactly once, before any log events.
pub fn init_logging() {
    let filter = build_env_filter();

    let subscriber = tracing_subscriber::fmt()
        // ── Output destination ────────────────────────────────────────────
        .with_writer(std::io::stdout)
        // ── Filter ────────────────────────────────────────────────────────
        .with_env_filter(filter)
        // ── Span / event metadata ─────────────────────────────────────────
        .with_target(true) // print the crate::module path
        .with_thread_ids(true) // helps correlate async tasks
        .with_file(true) // source file name
        .with_line_number(true) // source line number
        // ── Formatting ────────────────────────────────────────────────────
        .with_ansi(true) // ANSI colours; set false if piping to a file
        .pretty() // multi-line; swap for .compact() if preferred
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global tracing subscriber");
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build the [`EnvFilter`] from `RUST_LOG` (host filter) and `WASM_LOG`
/// (zome passthrough level).  Both are optional; the defaults are `host:warn`
/// and no wasm forwarding respectively.
fn build_env_filter() -> EnvFilter {
    // ── RUST_LOG (host filter) ────────────────────────────────────────────
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        eprintln!("[LOGGING] RUST_LOG not set, defaulting to 'host:warn'");
        "host:warn".to_string()
    });

    eprintln!("[LOGGING] RUST_LOG={}", rust_log);
    let mut expanded = expand_host_shorthands(&rust_log);
    eprintln!("[LOGGING] Host filter expanded: {}", expanded);

    // ── WASM_LOG (zome passthrough) ───────────────────────────────────────
    // Holochain reads WASM_LOG directly to gate what it forwards; we also add
    // the wasm targets to the host filter at the same level so events are not
    // dropped by tracing-subscriber after the conductor forwards them.
    match std::env::var("WASM_LOG") {
        Ok(wasm_level) => {
            eprintln!("[LOGGING] WASM_LOG={} — adding wasm targets to host filter", wasm_level);
            for target in WASM_TARGETS {
                expanded.push(',');
                expanded.push_str(&format!("{target}={wasm_level}"));
            }
        }
        Err(_) => {
            eprintln!("[LOGGING] WASM_LOG not set — zome logs will not be forwarded");
        }
    }

    eprintln!("[LOGGING] Final filter: {}", expanded);
    EnvFilter::from_str(&expanded).unwrap_or_else(|e| {
        eprintln!("[LOGGING] Invalid filter string ({e}), falling back to 'host:warn'");
        EnvFilter::from_str(&expand_host_shorthands("host:warn")).unwrap()
    })
}

/// Expand host-side shorthands in a comma-separated `RUST_LOG` value.
/// Tokens that are not recognised shorthands are passed through unchanged.
fn expand_host_shorthands(input: &str) -> String {
    let mut parts: Vec<String> = Vec::new();

    for token in input.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        // Shorthands use `key:level` (colon).  Standard directives use `crate=level` (equals).
        if let Some((key, level)) = token.split_once(':') {
            match key.trim() {
                // ── host:LEVEL ────────────────────────────────────────────
                // All project-owned host-side crates at LEVEL, plus the
                // standard holochain noise suppression directives.
                "host" => {
                    for krate in HOST_CRATES {
                        parts.push(format!("{krate}={level}"));
                    }
                    push_holochain_noise_filter(&mut parts);
                }

                // ── Unknown key — pass the whole token through unchanged ──
                _ => {
                    parts.push(token.to_string());
                }
            }
        } else {
            // No colon — raw tracing-subscriber directive, pass through.
            parts.push(token.to_string());
        }
    }

    parts.join(",")
}

/// Append the holochain noise-suppression directives to `parts`.
fn push_holochain_noise_filter(parts: &mut Vec<String>) {
    for (krate, level) in HOLOCHAIN_NOISY_CRATES {
        parts.push(format!("{krate}={level}"));
    }
}