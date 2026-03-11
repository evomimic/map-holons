use crate::wire::*;
use base_types::{BaseValue, MapString};
use core_types::PropertyName;
use holons_boundary::SmartReferenceWire;
use holons_core::core_shared_objects::transactions::TxId;

/// Construct a TxId for testing via serde roundtrip (TxId has no public constructor).
fn test_tx_id(val: u64) -> TxId {
    serde_json::from_value(serde_json::json!(val)).expect("TxId from u64")
}

/// Construct a test HolonId (local variant with dummy bytes).
fn test_holon_id() -> core_types::HolonId {
    core_types::HolonId::Local(integrity_core_types::LocalId(vec![0u8; 39]))
}

/// Helper: serialize to JSON and deserialize back, asserting roundtrip equality.
fn assert_roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug + PartialEq,
{
    let json = serde_json::to_string(value).expect("serialize failed");
    let restored: T = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(*value, restored, "roundtrip mismatch for: {json}");
}

// ── MapIpcRequest / MapIpcResponse ──────────────────────────────────

#[test]
fn roundtrip_ipc_request_space_command() {
    let request = MapIpcRequest {
        request_id: RequestId::new(42),
        command: MapCommandWire::Space(SpaceCommandWire::BeginTransaction),
    };
    assert_roundtrip(&request);
}

#[test]
fn roundtrip_ipc_response_success() {
    let response = MapIpcResponse {
        request_id: RequestId::new(7),
        result: Ok(MapResultWire::TransactionCreated {
            tx_id: test_tx_id(1),
        }),
    };
    assert_roundtrip(&response);
}

#[test]
fn roundtrip_ipc_response_error() {
    let response = MapIpcResponse {
        request_id: RequestId::new(7),
        result: Err(core_types::HolonError::NotImplemented(
            "test".to_string(),
        )),
    };
    assert_roundtrip(&response);
}

// ── SpaceCommandWire ────────────────────────────────────────────────

#[test]
fn roundtrip_space_begin_transaction() {
    let cmd = MapCommandWire::Space(SpaceCommandWire::BeginTransaction);
    assert_roundtrip(&cmd);
}

// ── TransactionCommandWire ──────────────────────────────────────────

#[test]
fn roundtrip_transaction_commit() {
    let cmd = MapCommandWire::Transaction(TransactionCommandWire {
        tx_id: test_tx_id(1),
        action: TransactionActionWire::Commit,
    });
    assert_roundtrip(&cmd);
}

#[test]
fn roundtrip_transaction_mutation_new_holon() {
    let cmd = MapCommandWire::Transaction(TransactionCommandWire {
        tx_id: test_tx_id(2),
        action: TransactionActionWire::Mutation(MutationActionWire::NewHolon {
            key: Some(MapString::from("my-key")),
        }),
    });
    assert_roundtrip(&cmd);
}

#[test]
fn roundtrip_transaction_lookup_get_all() {
    let cmd = MapCommandWire::Transaction(TransactionCommandWire {
        tx_id: test_tx_id(3),
        action: TransactionActionWire::Lookup(LookupActionWire::GetAllHolons),
    });
    assert_roundtrip(&cmd);
}

// ── HolonCommandWire ────────────────────────────────────────────────

#[test]
fn roundtrip_holon_read_property() {
    let tx_id = test_tx_id(1);
    let cmd = MapCommandWire::Holon(HolonCommandWire {
        tx_id,
        target: holons_boundary::HolonReferenceWire::Smart(SmartReferenceWire::new(
            tx_id,
            test_holon_id(),
            None,
        )),
        action: HolonActionWire::Read(ReadableHolonActionWire::PropertyValue {
            name: PropertyName(MapString::from("title")),
        }),
    });
    assert_roundtrip(&cmd);
}

#[test]
fn roundtrip_holon_write_property() {
    let tx_id = test_tx_id(1);
    let cmd = MapCommandWire::Holon(HolonCommandWire {
        tx_id,
        target: holons_boundary::HolonReferenceWire::Smart(SmartReferenceWire::new(
            tx_id,
            test_holon_id(),
            None,
        )),
        action: HolonActionWire::Write(WritableHolonActionWire::WithPropertyValue {
            name: PropertyName(MapString::from("title")),
            value: BaseValue::StringValue(MapString::from("hello")),
        }),
    });
    assert_roundtrip(&cmd);
}

// ── MapResultWire ───────────────────────────────────────────────────

#[test]
fn roundtrip_result_unit() {
    assert_roundtrip(&MapResultWire::Unit);
}

#[test]
fn roundtrip_result_transaction_created() {
    assert_roundtrip(&MapResultWire::TransactionCreated {
        tx_id: test_tx_id(5),
    });
}

#[test]
fn roundtrip_result_committed() {
    assert_roundtrip(&MapResultWire::Committed);
}
