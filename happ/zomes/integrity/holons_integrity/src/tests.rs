use super::*;
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use shared_validation::pvl_limits_v1::MAX_HOLON_NODE_BYTES;

fn oversized_store_record_update() -> Op {
    let entry_type =
        EntryType::App(AppEntryDef::new(EntryDefIndex(0), ZomeIndex(0), EntryVisibility::Public));
    let update = Update {
        author: AgentPubKey::from_raw_36(vec![0; 36]),
        timestamp: Timestamp::from_micros(1),
        action_seq: 2,
        prev_action: ActionHash::from_raw_36(vec![1; 36]),
        // This address is intentionally not backed by a record. Reaching
        // must_get_valid_record would therefore fail the test.
        original_action_address: ActionHash::from_raw_36(vec![2; 36]),
        original_entry_address: EntryHash::from_raw_36(vec![3; 36]),
        entry_type,
        entry_hash: EntryHash::from_raw_36(vec![4; 36]),
        weight: EntryRateWeight::default(),
    };
    let signed_action = SignedHashed::with_presigned(
        HoloHashed::with_pre_hashed(Action::Update(update), ActionHash::from_raw_36(vec![5; 36])),
        Signature([0; SIGNATURE_BYTES]),
    );
    let entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(vec![
        0xc1;
        MAX_HOLON_NODE_BYTES
            + 1
    ]))));

    Op::StoreRecord(StoreRecord { record: Record::new(signed_action, Some(entry)) })
}

#[test]
fn oversized_store_record_update_is_rejected_before_dependency_lookup() {
    match validate(oversized_store_record_update()) {
        Ok(ValidateCallbackResult::Invalid(message)) => {
            assert_eq!(message, "MAP-PVL-1003: HolonNode exceeds 262144-byte limit");
        }
        other => {
            panic!("expected the raw-size PVL rejection before dependency lookup, got {other:?}")
        }
    }
}
