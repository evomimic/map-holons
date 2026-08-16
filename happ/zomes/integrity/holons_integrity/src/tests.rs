use super::*;
use holochain_serialized_bytes::{encode, SerializedBytes, UnsafeBytes};
use mockall::mock;
use shared_validation::pvl_limits_v1::MAX_HOLON_NODE_BYTES;

const HOLON_ENTRY_DEF_INDEX: EntryDefIndex = EntryDefIndex(0);
const HOLON_ZOME_INDEX: ZomeIndex = ZomeIndex(0);
const ALL_HOLON_NODES_LINK_TYPE: LinkType = LinkType(LinkTypes::AllHolonNodes as u8);
const LOCAL_HOLON_SPACE_LINK_TYPE: LinkType = LinkType(LinkTypes::LocalHolonSpace as u8);
const SMARTLINK_LINK_TYPE: LinkType = LinkType(LinkTypes::SmartLink as u8);

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

#[derive(Clone, Copy, Debug)]
enum CreateLinkOpForm {
    RegisterCreateLink,
    StoreRecord,
}

#[derive(Clone, Copy, Debug)]
enum DeleteLinkOpForm {
    RegisterDeleteLink,
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

fn signed_create_link(action: CreateLink) -> SignedHashed<CreateLink> {
    SignedHashed::with_presigned(
        HoloHashed::with_pre_hashed(action, action_hash(14)),
        Signature([0; SIGNATURE_BYTES]),
    )
}

fn signed_delete_link(action: DeleteLink) -> SignedHashed<DeleteLink> {
    SignedHashed::with_presigned(
        HoloHashed::with_pre_hashed(action, action_hash(15)),
        Signature([0; SIGNATURE_BYTES]),
    )
}

fn signed_action(action: Action) -> SignedActionHashed {
    SignedHashed::with_presigned(
        HoloHashed::with_pre_hashed(action, action_hash(13)),
        Signature([0; SIGNATURE_BYTES]),
    )
}

fn agent_validation_package() -> Action {
    Action::AgentValidationPkg(AgentValidationPkg {
        author: AgentPubKey::from_raw_36(vec![0; 36]),
        timestamp: Timestamp::from_micros(0),
        action_seq: 0,
        prev_action: action_hash(30),
        membrane_proof: None,
    })
}

fn create_agent_activity_op() -> Op {
    let agent = AgentPubKey::from_raw_36(vec![31; 36]);
    let create = Create {
        author: agent.clone(),
        timestamp: Timestamp::from_micros(1),
        action_seq: 1,
        prev_action: action_hash(32),
        entry_type: EntryType::AgentPubKey,
        entry_hash: EntryHash::from(agent.clone()),
        weight: EntryRateWeight::default(),
    };
    Op::RegisterAgentActivity(RegisterAgentActivity {
        action: signed_action(Action::Create(create)),
        cached_entry: Some(Entry::Agent(agent)),
    })
}

fn holon_entry() -> Entry {
    let node = HolonNode::new(PropertyMap::new());
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

fn create_entry_op(store_record: bool) -> Op {
    let action = create_action();
    let entry = holon_entry();
    if store_record {
        Op::StoreRecord(StoreRecord {
            record: Record::new(signed_action(Action::Create(action)), Some(entry)),
        })
    } else {
        Op::StoreEntry(StoreEntry {
            action: signed_entry_creation_action(EntryCreationAction::Create(action)),
            entry,
        })
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
            // LinkTypes::from_type performs scoped resolution during both op
            // flattening and StoreRecord delete dispatch.
            links: ScopedZomeTypes(vec![(
                HOLON_ZOME_INDEX,
                vec![ALL_HOLON_NODES_LINK_TYPE, LOCAL_HOLON_SPACE_LINK_TYPE, SMARTLINK_LINK_TYPE],
            )]),
        },
    )
}

fn install_update_target(target_action: Action) {
    let target_record = Record::new(signed_action(target_action), None);
    let mut mock_hdi = MockHdi::new();
    mock_hdi
        .expect_must_get_valid_record()
        .withf(|input| input.0 == action_hash(4))
        .times(1)
        .return_once(move |_| Ok(target_record));
    mock_hdi.expect_must_get_action().times(0);
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
        .times(1)
        .return_once(move |_| Ok(target_action));
    mock_hdi.expect_must_get_valid_record().times(0);
    mock_hdi.expect_zome_info().times(0..=1).return_once(|_| Ok(zome_info()));
    set_hdi(mock_hdi);
}

#[test]
fn both_holon_node_create_forms_use_no_dependencies() {
    for store_record in [false, true] {
        let mut mock = MockHdi::new();
        mock.expect_must_get_action().times(0);
        mock.expect_must_get_valid_record().times(0);
        mock.expect_zome_info().times(0..=1).return_once(|_| Ok(zome_info()));
        set_hdi(mock);

        assert_eq!(validate(create_entry_op(store_record)), Ok(ValidateCallbackResult::Valid));
    }
}

#[test]
fn create_agent_routes_through_exactly_one_action_dependency() {
    let predecessor = signed_action(agent_validation_package());
    let mut mock = MockHdi::new();
    mock.expect_must_get_action()
        .withf(|input| input.0 == action_hash(32))
        .times(1)
        .return_once(move |_| Ok(predecessor));
    mock.expect_must_get_valid_record().times(0);
    set_hdi(mock);

    assert_eq!(validate(create_agent_activity_op()), Ok(ValidateCallbackResult::Valid));
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

fn create_link(link_type: LinkType, tag: LinkTag) -> CreateLink {
    CreateLink {
        author: AgentPubKey::from_raw_36(vec![0; 36]),
        timestamp: Timestamp::from_micros(4),
        action_seq: 4,
        prev_action: action_hash(7),
        base_address: action_hash(20).into(),
        target_address: action_hash(21).into(),
        zome_index: HOLON_ZOME_INDEX,
        link_type,
        tag,
        weight: RateWeight::default(),
    }
}

fn infrastructure_create_link(link_type: LinkType) -> CreateLink {
    let path = if link_type == ALL_HOLON_NODES_LINK_TYPE {
        ALL_HOLON_NODES_PATH
    } else {
        assert_eq!(link_type, LOCAL_HOLON_SPACE_LINK_TYPE);
        LOCAL_HOLON_SPACE_PATH
    };
    let mut create = create_link(link_type, LinkTag::new(Vec::new()));
    create.base_address = Path::from(path).path_entry_hash().expect("path hashing is local").into();
    create
}

fn create_link_op(form: CreateLinkOpForm, create: CreateLink) -> Op {
    match form {
        CreateLinkOpForm::RegisterCreateLink => {
            Op::RegisterCreateLink(RegisterCreateLink { create_link: signed_create_link(create) })
        }
        CreateLinkOpForm::StoreRecord => Op::StoreRecord(StoreRecord {
            record: Record::new(signed_action(Action::CreateLink(create)), None),
        }),
    }
}

fn delete_link_op(form: DeleteLinkOpForm, create: CreateLink) -> Op {
    match form {
        DeleteLinkOpForm::RegisterDeleteLink => Op::RegisterDeleteLink(RegisterDeleteLink {
            delete_link: signed_delete_link(delete_link()),
            create_link: create,
        }),
        DeleteLinkOpForm::StoreRecord => Op::StoreRecord(StoreRecord {
            record: Record::new(signed_action(Action::DeleteLink(delete_link())), None),
        }),
    }
}

fn delete_link() -> DeleteLink {
    DeleteLink {
        author: AgentPubKey::from_raw_36(vec![0; 36]),
        timestamp: Timestamp::from_micros(5),
        action_seq: 5,
        prev_action: action_hash(22),
        base_address: action_hash(20).into(),
        link_add_address: action_hash(23),
    }
}

fn valid_smartlink_tag() -> LinkTag {
    LinkTag::new(
        [[0xE2, 0x82, 0xB7].as_slice(), b"RelatedTo\0key\0".as_slice(), [1, 0].as_slice()].concat(),
    )
}

fn install_link_zome_info_without_dependencies() {
    let mut mock = MockHdi::new();
    mock.expect_zome_info().times(0..=1).return_once(|_| Ok(zome_info()));
    mock.expect_must_get_action().times(0);
    mock.expect_must_get_valid_record().times(0);
    set_hdi(mock);
}

fn install_infrastructure_create_target(expected_record_dependencies: usize) {
    let target = Record::new(signed_action(Action::Create(create_action())), None);
    let mut mock = MockHdi::new();
    mock.expect_must_get_action().times(0);
    mock.expect_must_get_valid_record()
        .times(expected_record_dependencies)
        .return_once(move |_| Ok(target));
    // Flattening and active-index target classification both need scoped type information. These
    // local metadata calls are not DHT dependencies and are deliberately outside the bound.
    mock.expect_zome_info().times(0..=2).returning(|_| Ok(zome_info()));
    set_hdi(mock);
}

#[test]
fn both_smartlink_create_forms_use_no_dependencies() {
    for form in [CreateLinkOpForm::RegisterCreateLink, CreateLinkOpForm::StoreRecord] {
        install_link_zome_info_without_dependencies();
        assert_eq!(
            validate(
                create_link_op(form, create_link(SMARTLINK_LINK_TYPE, valid_smartlink_tag()),)
            ),
            Ok(ValidateCallbackResult::Valid),
            "{form:?} bypassed SmartLink create validation"
        );
    }
}

#[test]
fn infrastructure_create_forms_pin_their_structural_dependency_counts() {
    for form in [CreateLinkOpForm::RegisterCreateLink, CreateLinkOpForm::StoreRecord] {
        for link_type in [ALL_HOLON_NODES_LINK_TYPE, LOCAL_HOLON_SPACE_LINK_TYPE] {
            install_infrastructure_create_target(1);
            assert_eq!(
                validate(create_link_op(form, infrastructure_create_link(link_type))),
                Ok(ValidateCallbackResult::Valid),
                "{form:?} did not route the active infrastructure create"
            );
        }
    }
}

#[test]
fn every_register_delete_link_form_uses_no_dependencies() {
    for link_type in [ALL_HOLON_NODES_LINK_TYPE, LOCAL_HOLON_SPACE_LINK_TYPE, SMARTLINK_LINK_TYPE] {
        let mut mock = MockHdi::new();
        mock.expect_must_get_action().times(0);
        mock.expect_must_get_valid_record().times(0);
        mock.expect_zome_info().times(0..=1).return_once(|_| Ok(zome_info()));
        set_hdi(mock);

        let result = validate(delete_link_op(
            DeleteLinkOpForm::RegisterDeleteLink,
            if link_type == SMARTLINK_LINK_TYPE {
                create_link(link_type, valid_smartlink_tag())
            } else {
                infrastructure_create_link(link_type)
            },
        ));
        let expected = if link_type == ALL_HOLON_NODES_LINK_TYPE {
            ValidateCallbackResult::Invalid("AllHolonNodes links cannot be deleted".into())
        } else {
            ValidateCallbackResult::Valid
        };
        assert_eq!(result, Ok(expected), "RegisterDeleteLink did not dispatch type {link_type:?}");
    }
}

#[test]
fn every_store_record_delete_link_form_uses_one_action_dependency() {
    for link_type in [ALL_HOLON_NODES_LINK_TYPE, LOCAL_HOLON_SPACE_LINK_TYPE, SMARTLINK_LINK_TYPE] {
        let create = if link_type == SMARTLINK_LINK_TYPE {
            create_link(link_type, valid_smartlink_tag())
        } else {
            infrastructure_create_link(link_type)
        };
        let resolved = signed_action(Action::CreateLink(create.clone()));
        let mut mock = MockHdi::new();
        mock.expect_must_get_action()
            .withf(|input| input.0 == action_hash(23))
            .times(1)
            .return_once(move |_| Ok(resolved));
        mock.expect_must_get_valid_record().times(0);
        mock.expect_zome_info().times(1).return_once(|_| Ok(zome_info()));
        set_hdi(mock);

        let result = validate(delete_link_op(DeleteLinkOpForm::StoreRecord, create));
        let expected = if link_type == ALL_HOLON_NODES_LINK_TYPE {
            ValidateCallbackResult::Invalid("AllHolonNodes links cannot be deleted".into())
        } else {
            ValidateCallbackResult::Valid
        };
        assert_eq!(
            result,
            Ok(expected),
            "StoreRecord DeleteLink did not dispatch type {link_type:?}"
        );
    }
}

#[test]
fn both_smartlink_create_forms_return_the_same_deterministic_rejection() {
    let malformed = LinkTag::new(vec![0; 3]);
    let create = create_link(SMARTLINK_LINK_TYPE, malformed);
    let operations = [
        Op::RegisterCreateLink(RegisterCreateLink {
            create_link: signed_create_link(create.clone()),
        }),
        Op::StoreRecord(StoreRecord {
            record: Record::new(signed_action(Action::CreateLink(create)), None),
        }),
    ];

    for operation in operations {
        install_link_zome_info_without_dependencies();
        assert_eq!(
            validate(operation),
            Ok(ValidateCallbackResult::Invalid(
                "MAP-PVL-2001: malformed SmartLink (invalid discriminant at TagHeader)".into()
            ))
        );
    }
}

#[test]
fn register_smartlink_delete_is_valid_without_dependency_lookup() {
    let create = create_link(SMARTLINK_LINK_TYPE, valid_smartlink_tag());
    let delete = delete_link();
    install_link_zome_info_without_dependencies();

    assert_eq!(
        validate(Op::RegisterDeleteLink(RegisterDeleteLink {
            delete_link: signed_delete_link(delete),
            create_link: create,
        })),
        Ok(ValidateCallbackResult::Valid)
    );
}

#[test]
fn store_record_smartlink_delete_resolves_once_and_dispatches() {
    let create = create_link(SMARTLINK_LINK_TYPE, valid_smartlink_tag());
    let mut mock = MockHdi::new();
    mock.expect_must_get_action()
        .withf(|input| input.0 == action_hash(23))
        .times(1)
        .return_once(move |_| Ok(signed_action(Action::CreateLink(create))));
    mock.expect_must_get_valid_record().times(0);
    mock.expect_zome_info().times(1).return_once(|_| Ok(zome_info()));
    set_hdi(mock);

    assert_eq!(
        validate(Op::StoreRecord(StoreRecord {
            record: Record::new(signed_action(Action::DeleteLink(delete_link())), None),
        })),
        Ok(ValidateCallbackResult::Valid)
    );
}

#[test]
fn store_record_link_delete_rejects_a_non_create_target_with_map_pvl_2004() {
    let original_delete = delete_link();
    let mut mock = MockHdi::new();
    mock.expect_must_get_action()
        .times(1)
        .return_once(move |_| Ok(signed_action(Action::DeleteLink(original_delete))));
    mock.expect_must_get_valid_record().times(0);
    mock.expect_zome_info().times(0);
    set_hdi(mock);

    assert_eq!(
        validate(Op::StoreRecord(StoreRecord {
            record: Record::new(signed_action(Action::DeleteLink(delete_link())), None),
        })),
        Ok(ValidateCallbackResult::Invalid("MAP-PVL-2004: link delete target is invalid".into()))
    );
}

#[test]
fn store_record_delete_dispatches_a_different_scoped_link_type() {
    let create = create_link(ALL_HOLON_NODES_LINK_TYPE, valid_smartlink_tag());
    let mut mock = MockHdi::new();
    mock.expect_must_get_action()
        .times(1)
        .return_once(move |_| Ok(signed_action(Action::CreateLink(create))));
    mock.expect_must_get_valid_record().times(0);
    mock.expect_zome_info().times(1).return_once(|_| Ok(zome_info()));
    set_hdi(mock);

    assert_eq!(
        validate(Op::StoreRecord(StoreRecord {
            record: Record::new(signed_action(Action::DeleteLink(delete_link())), None),
        })),
        Ok(ValidateCallbackResult::Invalid("AllHolonNodes links cannot be deleted".into()))
    );
}

fn assert_dependency_failure_stays_outer(operation: Op, marker: &str) {
    let error = validate(operation)
        .expect_err("dependency unavailability must not become a completed validation verdict");
    let message = error.to_string();
    assert!(message.contains(marker), "outer error lost dependency context: {message}");
    assert!(
        !message.contains("MAP-PVL"),
        "dependency failure was incorrectly represented as a PVL violation: {message}"
    );
}

#[test]
fn dependency_failures_remain_outer_across_callback_adapter_routes() {
    let mut update_mock = MockHdi::new();
    update_mock.expect_must_get_action().times(0);
    update_mock.expect_must_get_valid_record().times(1).return_once(|_| {
        Err(wasm_error!(WasmErrorInner::Guest("update dependency unavailable".into())))
    });
    update_mock.expect_zome_info().times(0..=1).return_once(|_| Ok(zome_info()));
    set_hdi(update_mock);
    assert_dependency_failure_stays_outer(
        update_op(UpdateOpForm::StoreRecord),
        "update dependency unavailable",
    );

    let mut delete_mock = MockHdi::new();
    delete_mock.expect_must_get_action().times(1).return_once(|_| {
        Err(wasm_error!(WasmErrorInner::Guest("delete dependency unavailable".into())))
    });
    delete_mock.expect_must_get_valid_record().times(0);
    delete_mock.expect_zome_info().times(0..=1).return_once(|_| Ok(zome_info()));
    set_hdi(delete_mock);
    assert_dependency_failure_stays_outer(
        delete_op(DeleteOpForm::RegisterDelete),
        "delete dependency unavailable",
    );

    let mut infrastructure_mock = MockHdi::new();
    infrastructure_mock.expect_must_get_action().times(0);
    infrastructure_mock.expect_must_get_valid_record().times(1).return_once(|_| {
        Err(wasm_error!(WasmErrorInner::Guest("infrastructure dependency unavailable".into())))
    });
    infrastructure_mock.expect_zome_info().times(0..=2).returning(|_| Ok(zome_info()));
    set_hdi(infrastructure_mock);
    assert_dependency_failure_stays_outer(
        create_link_op(
            CreateLinkOpForm::RegisterCreateLink,
            infrastructure_create_link(ALL_HOLON_NODES_LINK_TYPE),
        ),
        "infrastructure dependency unavailable",
    );

    let mut link_delete_mock = MockHdi::new();
    link_delete_mock.expect_must_get_action().times(1).return_once(|_| {
        Err(wasm_error!(WasmErrorInner::Guest("link delete dependency unavailable".into())))
    });
    link_delete_mock.expect_must_get_valid_record().times(0);
    link_delete_mock.expect_zome_info().times(0);
    set_hdi(link_delete_mock);
    assert_dependency_failure_stays_outer(
        delete_link_op(
            DeleteLinkOpForm::StoreRecord,
            create_link(SMARTLINK_LINK_TYPE, valid_smartlink_tag()),
        ),
        "link delete dependency unavailable",
    );

    let mut agent_mock = MockHdi::new();
    agent_mock.expect_must_get_action().times(1).return_once(|_| {
        Err(wasm_error!(WasmErrorInner::Guest("agent dependency unavailable".into())))
    });
    agent_mock.expect_must_get_valid_record().times(0);
    set_hdi(agent_mock);
    assert_dependency_failure_stays_outer(
        create_agent_activity_op(),
        "agent dependency unavailable",
    );
}
