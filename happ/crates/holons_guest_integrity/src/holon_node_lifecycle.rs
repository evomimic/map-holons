//! Holochain adapter for descriptor-independent `HolonNode` lifecycle validation.
//!
//! This module resolves the action named by an update or delete and projects
//! only the facts needed by `shared_validation`. Target entry content is never
//! requested or decoded: the action already carries both its lifecycle kind and
//! its entry type.
//!
//! Host and dependency-resolution failures remain in the outer
//! [`ExternResult`]. The inner result represents a completed PVL verdict, which
//! prevents temporarily unavailable dependencies from becoming deterministic
//! validation failures.

use hdi::prelude::*;
use integrity_core_types::PvlViolation;
use shared_validation::{
    validate_delete_target, validate_update_target, LifecycleTarget, TargetActionKind,
    TargetEntryKind,
};

use crate::holon_node_envelope::HOLON_NODE_ENTRY_DEF_INDEX;

/// Resolves and validates the lineage root named by a `HolonNode` update.
///
/// [`must_get_valid_record`] provides inductive validity for the structural
/// lineage root. MAP's root-addressed topology bounds this resolution at one
/// hop: the target must itself be a `Create`, never another `Update`.
///
/// `new_entry_def` comes from the already envelope-validated operation. Comparing
/// the complete [`AppEntryDef`] gives scoped entry identity without another host
/// call and prevents an equal entry-definition index in another integrity zome
/// from being mistaken for `HolonNode`.
pub fn validate_holon_node_update_target(
    original_action_hash: &ActionHash,
    new_entry_def: &AppEntryDef,
) -> ExternResult<Result<(), PvlViolation>> {
    let target_record = must_get_valid_record(original_action_hash.clone())?;
    let target = classify_target(target_record.action(), |target_entry_def| {
        target_entry_def == new_entry_def
    });

    Ok(validate_update_target(&target))
}

/// Resolves and validates the exact persisted version named by a `HolonNode` delete.
///
/// A delete carries no lineage structure forward, so [`must_get_action`] supplies
/// every fact the rule needs without fetching the target record or entry.
/// [`zome_info`] is a local host call used only to establish the defining
/// integrity-zome scope for `HolonNode`; it is not a DHT dependency lookup.
pub fn validate_holon_node_delete_target(
    deletes_address: &ActionHash,
) -> ExternResult<Result<(), PvlViolation>> {
    let target_action = must_get_action(deletes_address.clone())?;
    let holon_node_entry_def =
        AppEntryDef::new(HOLON_NODE_ENTRY_DEF_INDEX, zome_info()?.id, EntryVisibility::Public);
    let target = classify_target(target_action.action(), |target_entry_def| {
        target_entry_def == &holon_node_entry_def
    });

    Ok(validate_delete_target(&target))
}

/// Projects a resolved Holochain action into the closed pure lifecycle model.
///
/// The caller supplies scoped app-entry classification because the update path
/// derives scope from the new entry while the delete path derives it from
/// `zome_info`. Keeping resolution outside this helper makes it impossible for
/// pure validation to trigger additional host calls.
fn classify_target(
    action: &Action,
    is_holon_node: impl FnOnce(&AppEntryDef) -> bool,
) -> LifecycleTarget {
    let action_kind = match action {
        Action::Create(_) => TargetActionKind::Create,
        Action::Update(_) => TargetActionKind::Update,
        _ => TargetActionKind::Other,
    };
    let entry_kind = match action.entry_type() {
        Some(EntryType::App(entry_def)) if is_holon_node(entry_def) => TargetEntryKind::HolonNode,
        Some(EntryType::App(_)) => TargetEntryKind::OtherAppEntry,
        Some(_) => TargetEntryKind::NonAppEntry,
        None => TargetEntryKind::Absent,
    };

    LifecycleTarget { action_kind, entry_kind }
}

#[cfg(test)]
mod tests {
    use integrity_core_types::PvlViolation;
    use mockall::mock;
    use shared_validation::{
        CREATE_ACTION_KIND, CREATE_OR_UPDATE_ACTION_KIND, HOLON_NODE_ENTRY_KIND, OTHER_ACTION_KIND,
        OTHER_APP_ENTRY_KIND, UPDATE_ACTION_KIND,
    };

    use super::*;

    const HOLON_ZOME_INDEX: ZomeIndex = ZomeIndex(0);

    // HDI 0.7.1 exposes mockall behind its `mock` feature but does not generate
    // the documented `MockHdiT` type. Define the test double locally and install
    // it through the same thread-local `set_hdi` seam used by the host wrappers.
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

    fn action_hash(seed: u8) -> ActionHash {
        ActionHash::from_raw_36(vec![seed; 36])
    }

    fn app_entry_def(zome_index: ZomeIndex, entry_index: EntryDefIndex) -> AppEntryDef {
        AppEntryDef::new(entry_index, zome_index, EntryVisibility::Public)
    }

    fn holon_node_entry_def() -> AppEntryDef {
        app_entry_def(HOLON_ZOME_INDEX, HOLON_NODE_ENTRY_DEF_INDEX)
    }

    fn create_action(entry_type: EntryType) -> Create {
        Create {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(1),
            action_seq: 1,
            prev_action: action_hash(1),
            entry_type,
            entry_hash: EntryHash::from_raw_36(vec![2; 36]),
            weight: EntryRateWeight::default(),
        }
    }

    fn update_action(entry_type: EntryType) -> Update {
        Update {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(2),
            action_seq: 2,
            prev_action: action_hash(3),
            original_action_address: action_hash(4),
            original_entry_address: EntryHash::from_raw_36(vec![5; 36]),
            entry_type,
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

    fn signed_action(action: Action) -> SignedActionHashed {
        SignedHashed::with_presigned(
            HoloHashed::with_pre_hashed(action, action_hash(10)),
            Signature([0; SIGNATURE_BYTES]),
        )
    }

    fn record(action: Action) -> Record {
        Record::new(signed_action(action), None)
    }

    fn zome_info_for(zome_index: ZomeIndex) -> ZomeInfo {
        ZomeInfo::new(
            "holons_integrity".into(),
            zome_index,
            SerializedBytes::default(),
            EntryDefs(Vec::new()),
            Vec::new(),
            ScopedZomeTypesSet::default(),
        )
    }

    fn install_update_target(target_action: Action) {
        let target_record = record(target_action);
        let mut mock_hdi = MockHdi::new();
        mock_hdi.expect_must_get_valid_record().times(1).return_once(move |_| Ok(target_record));
        set_hdi(mock_hdi);
    }

    fn install_delete_target(target_action: Action) {
        let target_action = signed_action(target_action);
        let mut mock_hdi = MockHdi::new();
        mock_hdi.expect_must_get_action().times(1).return_once(move |_| Ok(target_action));
        mock_hdi.expect_zome_info().times(1).return_once(|_| Ok(zome_info_for(HOLON_ZOME_INDEX)));
        set_hdi(mock_hdi);
    }

    #[test]
    fn update_accepts_a_holon_node_create_root() {
        let entry_def = holon_node_entry_def();
        install_update_target(Action::Create(create_action(EntryType::App(entry_def.clone()))));

        assert_eq!(validate_holon_node_update_target(&action_hash(20), &entry_def), Ok(Ok(())));
    }

    #[test]
    fn update_rejects_an_update_target_even_when_it_carries_a_holon_node() {
        let entry_def = holon_node_entry_def();
        install_update_target(Action::Update(update_action(EntryType::App(entry_def.clone()))));

        assert_eq!(
            validate_holon_node_update_target(&action_hash(20), &entry_def),
            Ok(Err(PvlViolation::InvalidUpdateTarget {
                expected_target_kind: CREATE_ACTION_KIND.into(),
                actual_target_kind: UPDATE_ACTION_KIND.into(),
            }))
        );
    }

    #[test]
    fn update_rejects_other_action_kinds_before_entry_classification() {
        let entry_def = holon_node_entry_def();
        install_update_target(Action::Delete(delete_action()));

        assert_eq!(
            validate_holon_node_update_target(&action_hash(20), &entry_def),
            Ok(Err(PvlViolation::InvalidUpdateTarget {
                expected_target_kind: CREATE_ACTION_KIND.into(),
                actual_target_kind: OTHER_ACTION_KIND.into(),
            }))
        );
    }

    #[test]
    fn update_rejects_a_create_carrying_another_app_entry() {
        let entry_def = holon_node_entry_def();
        let other_entry_def =
            app_entry_def(HOLON_ZOME_INDEX, EntryDefIndex(HOLON_NODE_ENTRY_DEF_INDEX.0 + 1));
        install_update_target(Action::Create(create_action(EntryType::App(other_entry_def))));

        assert_eq!(
            validate_holon_node_update_target(&action_hash(20), &entry_def),
            Ok(Err(PvlViolation::InvalidUpdateTarget {
                expected_target_kind: HOLON_NODE_ENTRY_KIND.into(),
                actual_target_kind: OTHER_APP_ENTRY_KIND.into(),
            }))
        );
    }

    #[test]
    fn update_classification_includes_the_integrity_zome_scope() {
        let entry_def = holon_node_entry_def();
        let same_index_other_scope =
            app_entry_def(ZomeIndex(HOLON_ZOME_INDEX.0 + 1), HOLON_NODE_ENTRY_DEF_INDEX);
        install_update_target(Action::Create(create_action(EntryType::App(
            same_index_other_scope,
        ))));

        assert_eq!(
            validate_holon_node_update_target(&action_hash(20), &entry_def),
            Ok(Err(PvlViolation::InvalidUpdateTarget {
                expected_target_kind: HOLON_NODE_ENTRY_KIND.into(),
                actual_target_kind: OTHER_APP_ENTRY_KIND.into(),
            }))
        );
    }

    #[test]
    fn target_classification_distinguishes_non_app_and_absent_entries() {
        let non_app_target =
            classify_target(&Action::Create(create_action(EntryType::AgentPubKey)), |_| false);
        let absent_target = classify_target(&Action::Delete(delete_action()), |_| false);

        assert_eq!(non_app_target.entry_kind, TargetEntryKind::NonAppEntry);
        assert_eq!(absent_target.entry_kind, TargetEntryKind::Absent);
    }

    #[test]
    fn delete_accepts_create_and_update_holon_node_versions() {
        for target_action in [
            Action::Create(create_action(EntryType::App(holon_node_entry_def()))),
            Action::Update(update_action(EntryType::App(holon_node_entry_def()))),
        ] {
            install_delete_target(target_action);

            assert_eq!(validate_holon_node_delete_target(&action_hash(21)), Ok(Ok(())));
        }
    }

    #[test]
    fn delete_rejects_other_action_kinds() {
        install_delete_target(Action::Delete(delete_action()));

        assert_eq!(
            validate_holon_node_delete_target(&action_hash(21)),
            Ok(Err(PvlViolation::InvalidDeleteTarget {
                expected_target_kind: CREATE_OR_UPDATE_ACTION_KIND.into(),
                actual_target_kind: OTHER_ACTION_KIND.into(),
            }))
        );
    }

    #[test]
    fn delete_rejects_an_entry_outside_the_scoped_holon_node_identity() {
        let same_index_other_scope =
            app_entry_def(ZomeIndex(HOLON_ZOME_INDEX.0 + 1), HOLON_NODE_ENTRY_DEF_INDEX);
        install_delete_target(Action::Create(create_action(EntryType::App(
            same_index_other_scope,
        ))));

        assert_eq!(
            validate_holon_node_delete_target(&action_hash(21)),
            Ok(Err(PvlViolation::InvalidDeleteTarget {
                expected_target_kind: HOLON_NODE_ENTRY_KIND.into(),
                actual_target_kind: OTHER_APP_ENTRY_KIND.into(),
            }))
        );
    }

    #[test]
    fn update_dependency_failures_propagate_through_the_outer_result() {
        let mut mock_hdi = MockHdi::new();
        mock_hdi
            .expect_must_get_valid_record()
            .times(1)
            .return_once(|_| Err(wasm_error!(WasmErrorInner::Guest("update dependency".into()))));
        set_hdi(mock_hdi);

        let error = validate_holon_node_update_target(&action_hash(20), &holon_node_entry_def())
            .expect_err("dependency failure must remain an outer error");

        assert!(error.to_string().contains("update dependency"));
    }

    #[test]
    fn delete_dependency_failures_propagate_before_zome_classification() {
        let mut mock_hdi = MockHdi::new();
        mock_hdi
            .expect_must_get_action()
            .times(1)
            .return_once(|_| Err(wasm_error!(WasmErrorInner::Guest("delete dependency".into()))));
        // A failed dependency lookup must return before the local zome-info call.
        mock_hdi.expect_zome_info().times(0);
        set_hdi(mock_hdi);

        let error = validate_holon_node_delete_target(&action_hash(21))
            .expect_err("dependency failure must remain an outer error");

        assert!(error.to_string().contains("delete dependency"));
    }
}
