use holochain_client::CellId;
use holochain_types::prelude::ZomeName;
use holochain_types::signal::{Signal, SystemSignal};

/// Typed host-side signal produced by the decode adapter after `on_signal` fires.
#[derive(Debug, Clone)]
pub enum HostSignal {
    /// Decoded signal from the configured holons zome.
    Holons { cell_id: CellId, zome_name: ZomeName, signal: HolonsZomeSignal },
    /// App signal from any other zome — raw bytes preserved for callers.
    OtherApp { cell_id: CellId, zome_name: ZomeName, raw: Vec<u8> },
    /// Holochain system signal (e.g. countersigning outcomes).
    System(SystemSignal),
    /// App signal targeting the holons zome that failed msgpack decode.
    DecodeError { cell_id: CellId, zome_name: ZomeName, raw: Vec<u8>, error: String },
}

/// Host-side mirror of the holons coordinator `Signal` enum.
///
/// `LinkTypes` and `EntryTypes` are defined in the `holons_integrity` guest crate and cannot
/// be compiled on the host, so those fields are kept as `serde_json::Value` and deserialized
/// from msgpack via `rmp_serde`. The `action` field retains the same opaque treatment to
/// avoid pulling in guest-only HDK types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum HolonsZomeSignal {
    LinkCreated {
        action: serde_json::Value,
        link_type: serde_json::Value,
    },
    LinkDeleted {
        action: serde_json::Value,
        link_type: serde_json::Value,
    },
    EntryCreated {
        action: serde_json::Value,
        app_entry: serde_json::Value,
    },
    EntryUpdated {
        action: serde_json::Value,
        app_entry: serde_json::Value,
        original_app_entry: serde_json::Value,
    },
    EntryDeleted {
        action: serde_json::Value,
        original_app_entry: serde_json::Value,
    },
}

/// Decode a raw `holochain_types::signal::Signal` into a typed `HostSignal`.
///
/// App signals whose `zome_name` matches `target_zome` are decoded as `HolonsZomeSignal`
/// (msgpack via rmp_serde). Non-matching app signals become `OtherApp`. System signals
/// pass through unchanged. Decode failures become `DecodeError`.
pub fn decode_signal(signal: Signal, target_zome: &str) -> HostSignal {
    match signal {
        Signal::App { cell_id, zome_name, signal: app_signal } => {
            // ExternIO(Vec<u8>) — the inner bytes are msgpack from the zome
            let bytes: Vec<u8> = app_signal.into_inner().0;
            if zome_name.0.as_ref() == target_zome {
                match rmp_serde::from_slice::<HolonsZomeSignal>(&bytes) {
                    Ok(zs) => HostSignal::Holons { cell_id, zome_name, signal: zs },
                    Err(e) => HostSignal::DecodeError {
                        cell_id,
                        zome_name,
                        raw: bytes,
                        error: e.to_string(),
                    },
                }
            } else {
                HostSignal::OtherApp { cell_id, zome_name, raw: bytes }
            }
        }
        Signal::System(s) => HostSignal::System(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_types::signal::SystemSignal;
    use holochain_zome_types::prelude::EntryHash;

    fn make_cell_id() -> CellId {
        use holochain_zome_types::prelude::{AgentPubKey, DnaHash};
        CellId::new(DnaHash::from_raw_32(vec![0u8; 32]), AgentPubKey::from_raw_32(vec![0u8; 32]))
    }

    fn make_zome_name(name: &str) -> ZomeName {
        ZomeName::from(name)
    }

    fn make_app_signal(bytes: Vec<u8>) -> holochain_zome_types::prelude::AppSignal {
        use holochain_zome_types::prelude::ExternIO;
        holochain_zome_types::prelude::AppSignal::new(ExternIO(bytes))
    }

    #[test]
    fn system_signal_passes_through() {
        let entry_hash = EntryHash::from_raw_32(vec![0u8; 32]);
        let signal = Signal::System(SystemSignal::AbandonedCountersigning(entry_hash));
        match decode_signal(signal, "holons") {
            HostSignal::System(_) => {}
            other => panic!("expected System, got {:?}", other),
        }
    }

    #[test]
    fn non_target_zome_becomes_other_app() {
        let signal = Signal::App {
            cell_id: make_cell_id(),
            zome_name: make_zome_name("other_zome"),
            signal: make_app_signal(b"anything".to_vec()),
        };
        match decode_signal(signal, "holons") {
            HostSignal::OtherApp { .. } => {}
            other => panic!("expected OtherApp, got {:?}", other),
        }
    }

    #[test]
    fn garbage_bytes_become_decode_error() {
        let signal = Signal::App {
            cell_id: make_cell_id(),
            zome_name: make_zome_name("holons"),
            signal: make_app_signal(b"not valid msgpack for HolonsZomeSignal".to_vec()),
        };
        match decode_signal(signal, "holons") {
            HostSignal::DecodeError { .. } => {}
            other => panic!("expected DecodeError, got {:?}", other),
        }
    }

    #[test]
    fn valid_holons_signal_decodes() {
        // Build a HolonsZomeSignal and round-trip through msgpack
        let zs = HolonsZomeSignal::EntryCreated {
            action: serde_json::Value::Null,
            app_entry: serde_json::Value::Null,
        };
        let bytes = rmp_serde::to_vec_named(&zs).expect("encode");
        let signal = Signal::App {
            cell_id: make_cell_id(),
            zome_name: make_zome_name("holons"),
            signal: make_app_signal(bytes),
        };
        match decode_signal(signal, "holons") {
            HostSignal::Holons { .. } => {}
            other => panic!("expected Holons, got {:?}", other),
        }
    }
}
