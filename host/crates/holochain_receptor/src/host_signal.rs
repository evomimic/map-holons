use holochain_client::CellId;
use holochain_types::prelude::ZomeName;
use holochain_types::signal::{Signal, SystemSignal};
use integrity_core_types::{LocalId, PersistenceTimestamp};

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
/// Carries only MAP identifiers (`LocalId`), the action timestamp, and (for links)
/// the link type string. No holon state crosses the wire — `HolonNode`/`HolonNodeModel`
/// are intentionally absent. This type is adapter-internal; the public boundary is
/// [`ActionEvent`](crate::ActionEvent).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum HolonsZomeSignal {
    LinkCreated {
        action_id: LocalId,
        link_type: String,
        timestamp: PersistenceTimestamp,
    },
    LinkDeleted {
        action_id: LocalId,
        link_type: String,
        timestamp: PersistenceTimestamp,
    },
    HolonCreated {
        action_id: LocalId,
        affected_holon: LocalId,
        timestamp: PersistenceTimestamp,
    },
    HolonUpdated {
        action_id: LocalId,
        affected_holon: LocalId,
        previous_holon: LocalId,
        timestamp: PersistenceTimestamp,
    },
    HolonDeleted {
        action_id: LocalId,
        affected_holon: LocalId,
        previous_holon: LocalId,
        timestamp: PersistenceTimestamp,
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

    // ── helpers ──────────────────────────────────────────────────────────────

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

    /// Encode a `HolonsZomeSignal` to msgpack, wrap it as a Holochain `Signal::App`
    /// from the "holons" zome, then decode it on the host via `decode_signal`.
    fn roundtrip(zs: HolonsZomeSignal) -> HostSignal {
        let bytes = rmp_serde::to_vec_named(&zs).expect("msgpack encode");
        let signal = Signal::App {
            cell_id: make_cell_id(),
            zome_name: make_zome_name("holons"),
            signal: make_app_signal(bytes),
        };
        decode_signal(signal, "holons")
    }

    /// A realistic 39-byte `LocalId` (same length as a Holochain `ActionHash`).
    fn action_id(seed: u8) -> LocalId {
        LocalId(vec![seed; 39])
    }

    fn ts() -> PersistenceTimestamp {
        PersistenceTimestamp(1_700_000_000_000_000)
    }

    // ── routing ──────────────────────────────────────────────────────────────

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
        let bytes = rmp_serde::to_vec_named(&HolonsZomeSignal::HolonCreated {
            action_id: action_id(1),
            affected_holon: action_id(1),
            timestamp: ts(),
        })
        .unwrap();
        let signal = Signal::App {
            cell_id: make_cell_id(),
            zome_name: make_zome_name("some_other_zome"),
            signal: make_app_signal(bytes),
        };
        match decode_signal(signal, "holons") {
            HostSignal::OtherApp { zome_name, .. } => {
                assert_eq!(zome_name.0.as_ref(), "some_other_zome");
            }
            other => panic!("expected OtherApp, got {:?}", other),
        }
    }

    #[test]
    fn garbage_bytes_become_decode_error() {
        let signal = Signal::App {
            cell_id: make_cell_id(),
            zome_name: make_zome_name("holons"),
            signal: make_app_signal(b"not valid msgpack".to_vec()),
        };
        match decode_signal(signal, "holons") {
            HostSignal::DecodeError { .. } => {}
            other => panic!("expected DecodeError, got {:?}", other),
        }
    }

    #[test]
    fn cell_id_and_zome_name_are_preserved() {
        match roundtrip(HolonsZomeSignal::HolonCreated {
            action_id: action_id(1),
            affected_holon: action_id(1),
            timestamp: ts(),
        }) {
            HostSignal::Holons { zome_name, .. } => {
                assert_eq!(zome_name.0.as_ref(), "holons");
            }
            other => panic!("expected Holons, got {:?}", other),
        }
    }

    // ── HolonCreated ─────────────────────────────────────────────────────────

    #[test]
    fn holon_created_decodes_with_correct_fields() {
        match roundtrip(HolonsZomeSignal::HolonCreated {
            action_id: action_id(0xAB),
            affected_holon: action_id(0xAB),
            timestamp: ts(),
        }) {
            HostSignal::Holons {
                signal: HolonsZomeSignal::HolonCreated { action_id, affected_holon, timestamp },
                ..
            } => {
                assert_eq!(action_id.0, vec![0xAB; 39]);
                assert_eq!(affected_holon.0, vec![0xAB; 39]);
                assert_eq!(timestamp, ts());
            }
            other => panic!("expected HolonCreated, got {:?}", other),
        }
    }

    // ── HolonUpdated ─────────────────────────────────────────────────────────

    #[test]
    fn holon_updated_decodes_with_correct_fields() {
        match roundtrip(HolonsZomeSignal::HolonUpdated {
            action_id: action_id(0xBC),
            affected_holon: action_id(0x11),
            previous_holon: action_id(0x22),
            timestamp: ts(),
        }) {
            HostSignal::Holons {
                signal:
                    HolonsZomeSignal::HolonUpdated { action_id, affected_holon, previous_holon, .. },
                ..
            } => {
                assert_eq!(action_id.0, vec![0xBC; 39]);
                assert_eq!(affected_holon.0, vec![0x11; 39]);
                assert_eq!(previous_holon.0, vec![0x22; 39]);
            }
            other => panic!("expected HolonUpdated, got {:?}", other),
        }
    }

    // ── HolonDeleted ─────────────────────────────────────────────────────────

    #[test]
    fn holon_deleted_decodes_with_correct_fields() {
        match roundtrip(HolonsZomeSignal::HolonDeleted {
            action_id: action_id(0xCD),
            affected_holon: action_id(0x33),
            previous_holon: action_id(0x44),
            timestamp: ts(),
        }) {
            HostSignal::Holons {
                signal: HolonsZomeSignal::HolonDeleted { affected_holon, previous_holon, .. },
                ..
            } => {
                assert_eq!(affected_holon.0, vec![0x33; 39]);
                assert_eq!(previous_holon.0, vec![0x44; 39]);
            }
            other => panic!("expected HolonDeleted, got {:?}", other),
        }
    }

    // ── LinkCreated / LinkDeleted ─────────────────────────────────────────────

    #[test]
    fn link_created_decodes_with_correct_fields() {
        match roundtrip(HolonsZomeSignal::LinkCreated {
            action_id: action_id(0xDE),
            link_type: "SmartLink".to_string(),
            timestamp: ts(),
        }) {
            HostSignal::Holons {
                signal: HolonsZomeSignal::LinkCreated { action_id, link_type, .. },
                ..
            } => {
                assert_eq!(action_id.0, vec![0xDE; 39]);
                assert_eq!(link_type, "SmartLink");
            }
            other => panic!("expected LinkCreated, got {:?}", other),
        }
    }

    #[test]
    fn link_deleted_decodes_with_correct_fields() {
        match roundtrip(HolonsZomeSignal::LinkDeleted {
            action_id: action_id(0xEF),
            link_type: "AllHolonNodes".to_string(),
            timestamp: ts(),
        }) {
            HostSignal::Holons {
                signal: HolonsZomeSignal::LinkDeleted { link_type, .. }, ..
            } => {
                assert_eq!(link_type, "AllHolonNodes");
            }
            other => panic!("expected LinkDeleted, got {:?}", other),
        }
    }
}
