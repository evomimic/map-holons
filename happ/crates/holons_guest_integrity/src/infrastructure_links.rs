//! Fixed Holochain-aware validation for MAP's infrastructure link indexes.
//!
//! These links are storage topology, not descriptor-independent SmartLinks. Their validation
//! therefore produces stable completed callback rejections rather than `PvlViolation` values.
//! Required target records are resolved exactly once, and their action metadata is sufficient to
//! classify the entry: target entry content is deliberately never decoded.

use std::fmt;

use hdi::prelude::*;
use shared_validation::{TargetActionKind, TargetEntryKind};

use crate::action_target::classify_target;
use crate::holon_node::{ALL_HOLON_NODES_PATH, LOCAL_HOLON_SPACE_PATH};
use crate::holon_node_envelope::HOLON_NODE_ENTRY_DEF_INDEX;

/// A completed fixed-policy verdict for an infrastructure link.
///
/// This type intentionally has no MAP-PVL code: infrastructure indexes are Holochain storage
/// policy and are outside descriptor-independent PVL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InfrastructureLinkRejection {
    HolonNodeUpdatesCreate,
    NonCanonicalBase { link_name: &'static str, expected_path: &'static str },
    NonEmptyTag { link_name: &'static str },
    NonActionTarget { link_name: &'static str },
    NonRootHolonNodeTarget { link_name: &'static str },
    AllHolonNodesDelete,
}

impl fmt::Display for InfrastructureLinkRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HolonNodeUpdatesCreate => {
                formatter.write_str("HolonNodeUpdates links are obsolete and cannot be created")
            }
            Self::NonCanonicalBase { link_name, expected_path } => write!(
                formatter,
                "{link_name} links must use the canonical `{expected_path}` path base"
            ),
            Self::NonEmptyTag { link_name } => {
                write!(formatter, "{link_name} links must use an empty tag")
            }
            Self::NonActionTarget { link_name } => {
                write!(formatter, "{link_name} links must target an ActionHash")
            }
            Self::NonRootHolonNodeTarget { link_name } => write!(
                formatter,
                "{link_name} links must target a HolonNode lineage-root Create action"
            ),
            Self::AllHolonNodesDelete => {
                formatter.write_str("AllHolonNodes links cannot be deleted")
            }
        }
    }
}

/// Rejects the obsolete revision index without inspecting either endpoint.
pub fn validate_holon_node_updates_create(
    _base_address: &AnyLinkableHash,
    _target_address: &AnyLinkableHash,
    _tag: &LinkTag,
) -> ExternResult<Result<(), InfrastructureLinkRejection>> {
    Ok(Err(InfrastructureLinkRejection::HolonNodeUpdatesCreate))
}

/// Allows removal of obsolete revision-index links until Storage SL5 retires the link type.
pub fn validate_holon_node_updates_delete(
    _original_action: &CreateLink,
) -> ExternResult<Result<(), InfrastructureLinkRejection>> {
    Ok(Ok(()))
}

/// Validates a whole-space index link to one `HolonNode` lineage root.
///
/// The index represents each lineage once, so its target must be the lineage's `Create` action;
/// accepting an `Update` would let the generic path-authoring extern insert arbitrary versions
/// into an index whose readers expect roots.
pub fn validate_all_holon_nodes_create(
    base_address: &AnyLinkableHash,
    target_address: &AnyLinkableHash,
    tag: &LinkTag,
) -> ExternResult<Result<(), InfrastructureLinkRejection>> {
    validate_root_index_create(
        "AllHolonNodes",
        ALL_HOLON_NODES_PATH,
        base_address,
        target_address,
        tag,
    )
}

pub fn validate_all_holon_nodes_delete(
    _original_action: &CreateLink,
) -> ExternResult<Result<(), InfrastructureLinkRejection>> {
    // The whole-space index remains authoritative until Storage SL5 retires it. Consequently,
    // deleted lineages can leave stale index membership during this intentionally temporary era.
    Ok(Err(InfrastructureLinkRejection::AllHolonNodesDelete))
}

/// Validates the bootstrap path link to the designated local-space lineage root.
///
/// This path selects a lineage, not a movable exact-version pointer. Version selection within the
/// lineage belongs to version topology, so accepting an `Update` here would change the storage
/// contract rather than merely broaden valid infrastructure-link targets.
pub fn validate_local_holon_space_create(
    base_address: &AnyLinkableHash,
    target_address: &AnyLinkableHash,
    tag: &LinkTag,
) -> ExternResult<Result<(), InfrastructureLinkRejection>> {
    validate_root_index_create(
        "LocalHolonSpace",
        LOCAL_HOLON_SPACE_PATH,
        base_address,
        target_address,
        tag,
    )
}

/// Allows deletion because deleting the local-space holon removes this path link.
pub fn validate_local_holon_space_delete(
    _original_action: &CreateLink,
) -> ExternResult<Result<(), InfrastructureLinkRejection>> {
    Ok(Ok(()))
}

fn validate_root_index_create(
    link_name: &'static str,
    canonical_path: &'static str,
    base_address: &AnyLinkableHash,
    target_address: &AnyLinkableHash,
    tag: &LinkTag,
) -> ExternResult<Result<(), InfrastructureLinkRejection>> {
    // `path_entry_hash` hashes the serialized path locally. It performs no HDI host call or DHT
    // lookup, so this canonical-base check does not consume a validation dependency.
    let canonical_base: AnyLinkableHash = Path::from(canonical_path).path_entry_hash()?.into();
    if base_address != &canonical_base {
        return Ok(Err(InfrastructureLinkRejection::NonCanonicalBase {
            link_name,
            expected_path: canonical_path,
        }));
    }
    if !tag.0.is_empty() {
        return Ok(Err(InfrastructureLinkRejection::NonEmptyTag { link_name }));
    }
    let Some(target_action_hash) = target_address.clone().into_action_hash() else {
        return Ok(Err(InfrastructureLinkRejection::NonActionTarget { link_name }));
    };

    // Active access indexes require an inductively valid record. Its action already identifies
    // both action kind and app entry definition, so fetching or decoding entry content is needless.
    let target_record = must_get_valid_record(target_action_hash)?;
    let holon_node_entry_def =
        AppEntryDef::new(HOLON_NODE_ENTRY_DEF_INDEX, zome_info()?.id, EntryVisibility::Public);
    let target =
        classify_target(target_record.action(), |entry_def| entry_def == &holon_node_entry_def);
    if target.action_kind != TargetActionKind::Create
        || target.entry_kind != TargetEntryKind::HolonNode
    {
        return Ok(Err(InfrastructureLinkRejection::NonRootHolonNodeTarget { link_name }));
    }

    Ok(Ok(()))
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

    const HOLON_ZOME_INDEX: ZomeIndex = ZomeIndex(0);

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

    fn canonical_base(path: &'static str) -> AnyLinkableHash {
        Path::from(path).path_entry_hash().expect("path hashing is local").into()
    }

    fn holon_entry_def() -> AppEntryDef {
        AppEntryDef::new(HOLON_NODE_ENTRY_DEF_INDEX, HOLON_ZOME_INDEX, EntryVisibility::Public)
    }

    fn create_action(entry_type: EntryType) -> Action {
        Action::Create(Create {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(1),
            action_seq: 1,
            prev_action: action_hash(1),
            entry_type,
            entry_hash: EntryHash::from_raw_36(vec![2; 36]),
            weight: EntryRateWeight::default(),
        })
    }

    fn update_action(entry_type: EntryType) -> Action {
        Action::Update(Update {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(2),
            action_seq: 2,
            prev_action: action_hash(3),
            original_action_address: action_hash(4),
            original_entry_address: EntryHash::from_raw_36(vec![5; 36]),
            entry_type,
            entry_hash: EntryHash::from_raw_36(vec![6; 36]),
            weight: EntryRateWeight::default(),
        })
    }

    fn record(action: Action) -> Record {
        let signed = SignedHashed::with_presigned(
            HoloHashed::with_pre_hashed(action, action_hash(7)),
            Signature([0; SIGNATURE_BYTES]),
        );
        Record::new(signed, None)
    }

    fn zome_info_value() -> ZomeInfo {
        ZomeInfo::new(
            "holons_integrity".into(),
            HOLON_ZOME_INDEX,
            SerializedBytes::default(),
            EntryDefs(Vec::new()),
            Vec::new(),
            ScopedZomeTypesSet::default(),
        )
    }

    fn no_dependency_mock() -> MockHdi {
        let mut mock = MockHdi::new();
        mock.expect_must_get_action().times(0);
        mock.expect_must_get_valid_record().times(0);
        mock.expect_zome_info().times(0);
        mock
    }

    fn install_target(action: Action) {
        let target_record = record(action);
        let mut mock = MockHdi::new();
        mock.expect_must_get_action().times(0);
        mock.expect_must_get_valid_record().times(1).return_once(move |_| Ok(target_record));
        mock.expect_zome_info().times(1).return_once(|_| Ok(zome_info_value()));
        set_hdi(mock);
    }

    fn original_create_link() -> CreateLink {
        CreateLink {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(1),
            action_seq: 1,
            prev_action: action_hash(1),
            base_address: canonical_base(ALL_HOLON_NODES_PATH),
            target_address: action_hash(2).into(),
            zome_index: HOLON_ZOME_INDEX,
            link_type: LinkType(0),
            tag: LinkTag::new(Vec::new()),
            weight: RateWeight::default(),
        }
    }

    type CreateValidator = fn(
        &AnyLinkableHash,
        &AnyLinkableHash,
        &LinkTag,
    ) -> ExternResult<Result<(), InfrastructureLinkRejection>>;

    fn active_indexes() -> [(&'static str, &'static str, CreateValidator); 2] {
        [
            ("AllHolonNodes", ALL_HOLON_NODES_PATH, validate_all_holon_nodes_create),
            ("LocalHolonSpace", LOCAL_HOLON_SPACE_PATH, validate_local_holon_space_create),
        ]
    }

    #[test]
    fn holon_node_updates_create_rejects_without_dependencies_or_endpoint_inspection() {
        set_hdi(no_dependency_mock());
        let arbitrary: AnyLinkableHash = ExternalHash::from_raw_36(vec![8; 36]).into();

        assert_eq!(
            validate_holon_node_updates_create(&arbitrary, &arbitrary, &LinkTag::new(vec![1])),
            Ok(Err(InfrastructureLinkRejection::HolonNodeUpdatesCreate))
        );
    }

    #[test]
    fn active_indexes_accept_canonical_empty_tagged_create_targets_with_one_dependency() {
        for (_, path, validate) in active_indexes() {
            install_target(create_action(EntryType::App(holon_entry_def())));
            assert_eq!(
                validate(&canonical_base(path), &action_hash(9).into(), &LinkTag::new(Vec::new()),),
                Ok(Ok(()))
            );
        }
    }

    #[test]
    fn malformed_shapes_reject_before_dependency_resolution() {
        for (link_name, path, validate) in active_indexes() {
            set_hdi(no_dependency_mock());
            assert_eq!(
                validate(
                    &canonical_base("not_the_canonical_path"),
                    &action_hash(9).into(),
                    &LinkTag::new(Vec::new()),
                ),
                Ok(Err(InfrastructureLinkRejection::NonCanonicalBase {
                    link_name,
                    expected_path: path,
                }))
            );

            set_hdi(no_dependency_mock());
            assert_eq!(
                validate(&canonical_base(path), &action_hash(9).into(), &LinkTag::new(vec![1]),),
                Ok(Err(InfrastructureLinkRejection::NonEmptyTag { link_name }))
            );

            set_hdi(no_dependency_mock());
            let entry_target: AnyLinkableHash = EntryHash::from_raw_36(vec![9; 36]).into();
            assert_eq!(
                validate(&canonical_base(path), &entry_target, &LinkTag::new(Vec::new()),),
                Ok(Err(InfrastructureLinkRejection::NonActionTarget { link_name }))
            );
        }
    }

    #[test]
    fn active_indexes_reject_update_targets_even_when_the_entry_type_is_holon_node() {
        for (link_name, path, validate) in active_indexes() {
            install_target(update_action(EntryType::App(holon_entry_def())));
            assert_eq!(
                validate(&canonical_base(path), &action_hash(9).into(), &LinkTag::new(Vec::new()),),
                Ok(Err(InfrastructureLinkRejection::NonRootHolonNodeTarget { link_name }))
            );
        }
    }

    #[test]
    fn active_indexes_reject_actions_without_entries() {
        let dna_action = Action::Dna(Dna {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(1),
            hash: DnaHash::from_raw_36(vec![10; 36]),
        });
        for (link_name, path, validate) in active_indexes() {
            install_target(dna_action.clone());
            assert_eq!(
                validate(&canonical_base(path), &action_hash(9).into(), &LinkTag::new(Vec::new()),),
                Ok(Err(InfrastructureLinkRejection::NonRootHolonNodeTarget { link_name }))
            );
        }
    }

    #[test]
    fn active_indexes_reject_create_targets_with_another_entry_type() {
        let other_entry = AppEntryDef::new(
            EntryDefIndex(HOLON_NODE_ENTRY_DEF_INDEX.0 + 1),
            HOLON_ZOME_INDEX,
            EntryVisibility::Public,
        );
        for (link_name, path, validate) in active_indexes() {
            install_target(create_action(EntryType::App(other_entry.clone())));
            assert_eq!(
                validate(&canonical_base(path), &action_hash(9).into(), &LinkTag::new(Vec::new()),),
                Ok(Err(InfrastructureLinkRejection::NonRootHolonNodeTarget { link_name }))
            );
        }
    }

    #[test]
    fn active_indexes_include_integrity_zome_scope_in_target_classification() {
        let same_index_other_scope = AppEntryDef::new(
            HOLON_NODE_ENTRY_DEF_INDEX,
            ZomeIndex(HOLON_ZOME_INDEX.0 + 1),
            EntryVisibility::Public,
        );
        for (link_name, path, validate) in active_indexes() {
            install_target(create_action(EntryType::App(same_index_other_scope.clone())));
            assert_eq!(
                validate(&canonical_base(path), &action_hash(9).into(), &LinkTag::new(Vec::new()),),
                Ok(Err(InfrastructureLinkRejection::NonRootHolonNodeTarget { link_name }))
            );
        }
    }

    #[test]
    fn dependency_failure_remains_an_outer_error() {
        let mut mock = MockHdi::new();
        mock.expect_must_get_action().times(0);
        mock.expect_must_get_valid_record().times(1).return_once(|_| {
            Err(wasm_error!(WasmErrorInner::Guest("infrastructure dependency".into())))
        });
        mock.expect_zome_info().times(0);
        set_hdi(mock);

        let error = validate_all_holon_nodes_create(
            &canonical_base(ALL_HOLON_NODES_PATH),
            &action_hash(9).into(),
            &LinkTag::new(Vec::new()),
        )
        .expect_err("dependency failures must not become completed invalid verdicts");

        assert!(error.to_string().contains("infrastructure dependency"));
    }

    #[test]
    fn fixed_delete_policies_use_no_dependencies() {
        let original = original_create_link();

        set_hdi(no_dependency_mock());
        assert_eq!(validate_holon_node_updates_delete(&original), Ok(Ok(())));
        set_hdi(no_dependency_mock());
        assert_eq!(validate_local_holon_space_delete(&original), Ok(Ok(())));
        set_hdi(no_dependency_mock());
        assert_eq!(
            validate_all_holon_nodes_delete(&original),
            Ok(Err(InfrastructureLinkRejection::AllHolonNodesDelete))
        );
    }
}
