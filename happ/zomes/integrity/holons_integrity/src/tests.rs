use super::*;
use holochain_serialized_bytes::{encode, SerializedBytes, UnsafeBytes};
use mockall::mock;
use shared_validation::pvl_limits_v1::MAX_HOLON_NODE_BYTES;

const HOLON_ENTRY_DEF_INDEX: EntryDefIndex = EntryDefIndex(0);
const HOLON_ZOME_INDEX: ZomeIndex = ZomeIndex(0);

// HDI 0.7.1 exposes mockall behind its `mock` feature but does not generate
// the documented `MockHdiT` type. Keep this callback-level test double local
// and install it through the same thread-local seam used by the guest adapter.
mock! {
    Hdi {}

    impl HdiT for Hdi {
        fn verify_signature(&self, input: VerifySignature) -> ExternResult<bool>;
        fn must_get_entry(&self, input: MustGetEntryInput) -> ExternResult<EntryHashed>;
        fn must_get_action(
            &self,
            input: MustGetActionInput,
        ) -> ExternResult<SignedActionHashed>;
        fn must_get_valid_record(
            &self,
            input: MustGetValidRecordInput,
        ) -> ExternResult<Record>;
        fn must_get_agent_activity(
            &self,
            input: MustGetAgentActivityInput,
        ) -> ExternResult<Vec<RegisterAgentActivity>>;
        fn dna_info(&self, input: ()) -> ExternResult<DnaInfo>;
        fn zome_info(&self, input: ()) -> ExternResult<ZomeInfo>;
        fn trace(&self, input: TraceMsg) -> ExternResult<()>;
        fn x_salsa20_poly1305_decrypt(
            &self,
            input: XSalsa20Poly1305Decrypt,
        ) -> ExternResult<Option<XSalsa20Poly1305Data>>;
        fn x_25519_x_salsa20_poly1305_decrypt(
            &self,
            input: X25519XSalsa20Poly1305Decrypt,
        ) -> ExternResult<Option<XSalsa20Poly1305Data>>;
        fn ed_25519_x_salsa20_poly1305_decrypt(
            &self,
            input: Ed25519XSalsa20Poly1305Decrypt,
        ) -> ExternResult<XSalsa20Poly1305Data>;
    }
}

#[derive(Clone, Copy, Debug)]
enum UpdateOpForm {
    StoreEntry,
    RegisterUpdate,
    StoreRecord,
}

#[derive(Clone, Copy, Debug)]
enum DeleteOpForm {
    RegisterDelete,
    StoreRecord,
}

fn action_hash(seed: u8) -> ActionHash {
    ActionHash::from_raw_36(vec![seed; 36])
}

fn holon_entry_type() -> EntryType {
    EntryType::App(AppEntryDef::new(
        HOLON_ENTRY_DEF_INDEX,
        HOLON_ZOME_INDEX,
        EntryVisibility::Public,
    ))
}

fn create_action() -> Create {
    Create {
        author: AgentPubKey::from_raw_36(vec![0; 36]),
        timestamp: Timestamp::from_micros(1),
        action_seq: 1,
        prev_action: action_hash(1),
        entry_type: holon_entry_type(),
        entry_hash: EntryHash::from_raw_36(vec![2; 36]),
        weight: EntryRateWeight::default(),
    }
}

fn update_action() -> Update {
    Update {
        author: AgentPubKey::from_raw_36(vec![0; 36]),
        timestamp: Timestamp::from_micros(2),
        action_seq: 2,
        prev_action: action_hash(3),
        original_action_address: action_hash(4),
        original_entry_address: EntryHash::from_raw_36(vec![5; 36]),
        entry_type: holon_entry_type(),
        entry_hash: EntryHash::from_raw_36(vec![6; 36]),
        weight: EntryRateWeight::default(),
    }
}

fn delete_action() -> Delete {
    Delete {
        author: AgentPubKey::from_raw_36(vec![0; 36]),
        timestamp: Timestamp::from_micros(3),
        action_seq: 3,
        prev_action: action_hash(7),
        deletes_address: action_hash(8),
        deletes_entry_address: EntryHash::from_raw_36(vec![9; 36]),
        weight: RateWeight::default(),
    }
}

fn signed_entry_creation_action(action: EntryCreationAction) -> SignedHashed<EntryCreationAction> {
    SignedHashed::with_presigned(
        HoloHashed::with_pre_hashed(action, action_hash(10)),
        Signature([0; SIGNATURE_BYTES]),
    )
}

fn signed_update(action: Update) -> SignedHashed<Update> {
    SignedHashed::with_presigned(
        HoloHashed::with_pre_hashed(action, action_hash(11)),
        Signature([0; SIGNATURE_BYTES]),
    )
}

fn signed_delete(action: Delete) -> SignedHashed<Delete> {
    SignedHashed::with_presigned(
        HoloHashed::with_pre_hashed(action, action_hash(12)),
        Signature([0; SIGNATURE_BYTES]),
    )
}

fn signed_action(action: Action) -> SignedActionHashed {
    SignedHashed::with_presigned(
        HoloHashed::with_pre_hashed(action, action_hash(13)),
        Signature([0; SIGNATURE_BYTES]),
    )
}

fn holon_entry() -> Entry {
    let node = HolonNode::new(None, PropertyMap::new());
    let raw = encode(&node).expect("test HolonNode must encode canonically");
    Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(raw))))
}

fn update_op(form: UpdateOpForm) -> Op {
    let action = update_action();
    let entry = holon_entry();

    match form {
        UpdateOpForm::StoreEntry => Op::StoreEntry(StoreEntry {
            action: signed_entry_creation_action(EntryCreationAction::Update(action)),
            entry,
        }),
        UpdateOpForm::RegisterUpdate => Op::RegisterUpdate(RegisterUpdate {
            update: signed_update(action),
            new_entry: Some(entry),
        }),
        UpdateOpForm::StoreRecord => Op::StoreRecord(StoreRecord {
            record: Record::new(signed_action(Action::Update(action)), Some(entry)),
        }),
    }
}

fn delete_op(form: DeleteOpForm) -> Op {
    let action = delete_action();

    match form {
        DeleteOpForm::RegisterDelete => {
            Op::RegisterDelete(RegisterDelete { delete: signed_delete(action) })
        }
        DeleteOpForm::StoreRecord => Op::StoreRecord(StoreRecord {
            record: Record::new(signed_action(Action::Delete(action)), None),
        }),
    }
}

fn zome_info() -> ZomeInfo {
    ZomeInfo::new(
        "holons_integrity".into(),
        HOLON_ZOME_INDEX,
        SerializedBytes::default(),
        EntryDefs(Vec::new()),
        Vec::new(),
        ScopedZomeTypesSet {
            entries: ScopedZomeTypes(vec![(HOLON_ZOME_INDEX, vec![HOLON_ENTRY_DEF_INDEX])]),
            links: ScopedZomeTypes(Vec::new()),
        },
    )
}

fn install_update_target(target_action: Action) {
    let target_record = Record::new(signed_action(target_action), None);
    let mut mock_hdi = MockHdi::new();
    mock_hdi
        .expect_must_get_valid_record()
        .withf(|input| input.0 == action_hash(4))
        .times(0..=1)
        .return_once(move |_| Ok(target_record));
    // Op flattening resolves this zome's generated EntryTypes before the
    // lifecycle adapter runs. The update adapter itself makes no zome-info call.
    mock_hdi.expect_zome_info().times(0..=1).return_once(|_| Ok(zome_info()));
    set_hdi(mock_hdi);
}

fn install_delete_target(target_action: Action) {
    let target_action = signed_action(target_action);
    let mut mock_hdi = MockHdi::new();
    mock_hdi
        .expect_must_get_action()
        .withf(|input| input.0 == action_hash(8))
        .times(0..=1)
        .return_once(move |_| Ok(target_action));
    mock_hdi.expect_zome_info().times(0..=1).return_once(|_| Ok(zome_info()));
    set_hdi(mock_hdi);
}

#[test]
fn all_three_update_arms_route_to_the_root_addressed_lifecycle_rule() {
    for form in [UpdateOpForm::StoreEntry, UpdateOpForm::RegisterUpdate, UpdateOpForm::StoreRecord]
    {
        install_update_target(Action::Update(update_action()));

        assert_eq!(
            validate(update_op(form)),
            Ok(ValidateCallbackResult::Invalid("MAP-PVL-1301: update target is invalid".into())),
            "{form:?} did not route through update lifecycle validation"
        );
    }
}

#[test]
fn all_three_update_arms_accept_a_holon_node_create_root() {
    for form in [UpdateOpForm::StoreEntry, UpdateOpForm::RegisterUpdate, UpdateOpForm::StoreRecord]
    {
        install_update_target(Action::Create(create_action()));

        assert_eq!(
            validate(update_op(form)),
            Ok(ValidateCallbackResult::Valid),
            "{form:?} rejected a valid HolonNode Create root"
        );
    }
}

#[test]
fn both_delete_arms_route_to_the_exact_version_lifecycle_rule() {
    for form in [DeleteOpForm::RegisterDelete, DeleteOpForm::StoreRecord] {
        install_delete_target(Action::Delete(delete_action()));

        assert_eq!(
            validate(delete_op(form)),
            Ok(ValidateCallbackResult::Invalid("MAP-PVL-1303: delete target is invalid".into())),
            "{form:?} did not route through delete lifecycle validation"
        );
    }
}

#[test]
fn both_delete_arms_accept_a_holon_node_create_version() {
    for form in [DeleteOpForm::RegisterDelete, DeleteOpForm::StoreRecord] {
        install_delete_target(Action::Create(create_action()));

        assert_eq!(
            validate(delete_op(form)),
            Ok(ValidateCallbackResult::Valid),
            "{form:?} rejected a valid HolonNode Create version"
        );
    }
}

fn oversized_store_record_update() -> Op {
    let update = update_action();
    let signed_action = signed_action(Action::Update(update));
    let entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(vec![
        0xc1;
        MAX_HOLON_NODE_BYTES
            + 1
    ]))));

    Op::StoreRecord(StoreRecord { record: Record::new(signed_action, Some(entry)) })
}

#[test]
fn oversized_store_record_update_is_rejected_before_dependency_lookup() {
    let mut mock_hdi = MockHdi::new();
    // Any dependency request would be an ordering regression: the raw envelope
    // guard must reject oversized bytes before lifecycle resolution begins.
    mock_hdi.expect_must_get_valid_record().times(0);
    set_hdi(mock_hdi);

    match validate(oversized_store_record_update()) {
        Ok(ValidateCallbackResult::Invalid(message)) => {
            assert_eq!(message, "MAP-PVL-1003: HolonNode exceeds 262144-byte limit");
        }
        other => {
            panic!("expected the raw-size PVL rejection before dependency lookup, got {other:?}")
        }
    }
}
