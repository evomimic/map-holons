//! Holochain adapter for descriptor-independent SmartLink validation.
//!
//! The shared validator owns Tag v1 parsing and consensus rules. This module
//! adds only the substrate facts that pure validation cannot establish: exact
//! Holochain hash kinds, outbound-proxy hash encoding, and delete-target action
//! resolution.
//!
//! Host and dependency failures remain in the outer [`ExternResult`]. The
//! inner result is a completed deterministic PVL verdict.

use hdi::prelude::*;
use integrity_core_types::PvlViolation;
use shared_validation::{
    validate_link_delete_target, validate_smartlink_envelope, LinkDeleteTargetKind,
    INVALID_ACTION_HASH_ENCODING, SMARTLINK_BASE_ENDPOINT, SMARTLINK_LINK_TARGET_ENDPOINT,
    SMARTLINK_OUTBOUND_PROXY_ENDPOINT,
};

use crate::local_id_from_action_hash;

const ACTION_HASH_KIND: &str = "ActionHash";
const ENTRY_HASH_KIND: &str = "EntryHash";
const EXTERNAL_HASH_KIND: &str = "ExternalHash";
const UNKNOWN_HASH_KIND: &str = "Unknown";

/// Validates a SmartLink create without resolving either endpoint from the DHT.
///
/// Exact Holochain hash-kind classification happens before the substrate-free
/// validator checks raw endpoint shape and decodes the tag. The returned
/// decoded envelope is retained long enough to validate a present outbound
/// proxy, avoiding a second Tag v1 decode.
pub fn validate_smartlink_create(
    base_address: &AnyLinkableHash,
    target_address: &AnyLinkableHash,
    tag: &LinkTag,
) -> ExternResult<Result<(), PvlViolation>> {
    let base_action_hash = match require_action_hash(base_address, SMARTLINK_BASE_ENDPOINT) {
        Ok(hash) => hash,
        Err(violation) => return Ok(Err(violation)),
    };
    let target_action_hash =
        match require_action_hash(target_address, SMARTLINK_LINK_TARGET_ENDPOINT) {
            Ok(hash) => hash,
            Err(violation) => return Ok(Err(violation)),
        };

    let base_local_id = local_id_from_action_hash(base_action_hash);
    let target_local_id = local_id_from_action_hash(target_action_hash);
    let decoded = match validate_smartlink_envelope(&base_local_id, &target_local_id, tag.as_ref())
    {
        Ok(decoded) => decoded,
        Err(violation) => return Ok(Err(violation)),
    };

    if let Some(external_id) = decoded.target_id.external_id() {
        let outbound_proxy_bytes = external_id.space_id.0.as_bytes().to_vec();
        if ActionHash::try_from_raw_39(outbound_proxy_bytes).is_err() {
            return Ok(Err(PvlViolation::InvalidSmartLinkEndpoint {
                endpoint: SMARTLINK_OUTBOUND_PROXY_ENDPOINT.into(),
                reason: INVALID_ACTION_HASH_ENCODING.into(),
            }));
        }
    }

    Ok(Ok(()))
}

/// Resolves and classifies the action named by a `DeleteLink`.
///
/// Exactly one [`must_get_action`] supplies every fact needed for classification
/// and later scoped link-type dispatch. An unresolved dependency remains an
/// outer host error and therefore cannot become a permanent PVL rejection.
pub fn resolve_link_delete_target(
    original_action_hash: ActionHash,
) -> ExternResult<Result<CreateLink, PvlViolation>> {
    let target_action = must_get_action(original_action_hash)?;
    let target_kind = match target_action.action() {
        Action::CreateLink(_) => LinkDeleteTargetKind::CreateLink,
        Action::DeleteLink(_) => LinkDeleteTargetKind::DeleteLink,
        _ => LinkDeleteTargetKind::Other,
    };
    if let Err(violation) = validate_link_delete_target(target_kind) {
        return Ok(Err(violation));
    }

    let Action::CreateLink(create_link) = target_action.action() else {
        return Err(wasm_error!(WasmErrorInner::Guest(
            "link-delete target classification accepted a non-CreateLink action".into()
        )));
    };

    Ok(Ok(create_link.clone()))
}

/// Validates deletion of an already-valid SmartLink create action.
///
/// Structural validity was established when the original create was accepted,
/// so deletion neither resolves dependencies nor decodes the original tag.
pub fn validate_smartlink_delete(
    _original_action: &CreateLink,
) -> ExternResult<Result<(), PvlViolation>> {
    Ok(Ok(()))
}

fn require_action_hash(hash: &AnyLinkableHash, endpoint: &str) -> Result<ActionHash, PvlViolation> {
    hash.clone().into_action_hash().ok_or_else(|| PvlViolation::UnsupportedSmartLinkEndpointKind {
        endpoint: endpoint.into(),
        endpoint_kind: any_linkable_hash_kind(hash).into(),
    })
}

fn any_linkable_hash_kind(hash: &AnyLinkableHash) -> &'static str {
    if hash.clone().into_action_hash().is_some() {
        ACTION_HASH_KIND
    } else if hash.clone().into_entry_hash().is_some() {
        ENTRY_HASH_KIND
    } else if hash.clone().into_external_hash().is_some() {
        EXTERNAL_HASH_KIND
    } else {
        UNKNOWN_HASH_KIND
    }
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

    // HDI 0.7.1 exposes mockall through its `mock` feature but does not
    // generate a reusable `MockHdiT`. Keep this adapter-specific double local
    // so dependency-call expectations stay visible beside the behavior.
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

    fn valid_tag() -> LinkTag {
        LinkTag::new(
            [[0xE2, 0x82, 0xB7].as_slice(), b"RelatedTo\0key\0".as_slice(), [1, 0].as_slice()]
                .concat(),
        )
    }

    fn create_link() -> CreateLink {
        CreateLink {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(1),
            action_seq: 1,
            prev_action: action_hash(1),
            base_address: action_hash(2).into(),
            target_address: action_hash(3).into(),
            zome_index: ZomeIndex(0),
            link_type: LinkType(3),
            tag: valid_tag(),
            weight: RateWeight::default(),
        }
    }

    fn signed_action(action: Action) -> SignedActionHashed {
        SignedHashed::with_presigned(
            HoloHashed::with_pre_hashed(action, action_hash(9)),
            Signature([0; SIGNATURE_BYTES]),
        )
    }

    fn no_dependency_mock() -> MockHdi {
        let mut mock = MockHdi::new();
        mock.expect_must_get_action().times(0);
        mock.expect_must_get_valid_record().times(0);
        mock
    }

    #[test]
    fn valid_action_hash_endpoints_pass_without_dependencies() {
        set_hdi(no_dependency_mock());

        assert_eq!(
            validate_smartlink_create(&action_hash(1).into(), &action_hash(2).into(), &valid_tag(),),
            Ok(Ok(()))
        );
    }

    #[test]
    fn unsupported_endpoint_kinds_are_classified_by_role_and_kind() {
        let entry: AnyLinkableHash = EntryHash::from_raw_36(vec![1; 36]).into();
        let external: AnyLinkableHash = ExternalHash::from_raw_36(vec![2; 36]).into();
        let action: AnyLinkableHash = action_hash(3).into();

        for (base, target, endpoint, endpoint_kind) in [
            (&entry, &action, SMARTLINK_BASE_ENDPOINT, ENTRY_HASH_KIND),
            (&external, &action, SMARTLINK_BASE_ENDPOINT, EXTERNAL_HASH_KIND),
            (&action, &entry, SMARTLINK_LINK_TARGET_ENDPOINT, ENTRY_HASH_KIND),
            (&action, &external, SMARTLINK_LINK_TARGET_ENDPOINT, EXTERNAL_HASH_KIND),
        ] {
            set_hdi(no_dependency_mock());
            assert_eq!(
                validate_smartlink_create(base, target, &valid_tag()),
                Ok(Err(PvlViolation::UnsupportedSmartLinkEndpointKind {
                    endpoint: endpoint.into(),
                    endpoint_kind: endpoint_kind.into(),
                }))
            );
        }
    }

    #[test]
    fn malformed_decoded_outbound_proxy_is_an_endpoint_violation() {
        let malformed_proxy_tag = LinkTag::new(
            [
                [0xE2, 0x82, 0xB7].as_slice(),
                b"RelatedTo\0key\0".as_slice(),
                [1, 1].as_slice(),
                [0; 39].as_slice(),
            ]
            .concat(),
        );
        set_hdi(no_dependency_mock());

        assert_eq!(
            validate_smartlink_create(
                &action_hash(1).into(),
                &action_hash(2).into(),
                &malformed_proxy_tag,
            ),
            Ok(Err(PvlViolation::InvalidSmartLinkEndpoint {
                endpoint: SMARTLINK_OUTBOUND_PROXY_ENDPOINT.into(),
                reason: INVALID_ACTION_HASH_ENCODING.into(),
            }))
        );
    }

    #[test]
    fn smartlink_delete_uses_no_dependencies_or_tag_decode() {
        set_hdi(no_dependency_mock());
        assert_eq!(validate_smartlink_delete(&create_link()), Ok(Ok(())));
    }

    #[test]
    fn delete_target_resolution_uses_one_action_lookup_and_no_record_lookup() {
        let expected = create_link();
        let returned = expected.clone();
        let mut mock = MockHdi::new();
        mock.expect_must_get_action()
            .times(1)
            .return_once(move |_| Ok(signed_action(Action::CreateLink(returned))));
        mock.expect_must_get_valid_record().times(0);
        set_hdi(mock);

        assert_eq!(resolve_link_delete_target(action_hash(7)), Ok(Ok(expected)));
    }

    #[test]
    fn dependency_failure_stays_outer_and_non_create_target_is_deterministic() {
        let mut failing = MockHdi::new();
        failing.expect_must_get_action().times(1).return_once(|_| {
            Err(wasm_error!(WasmErrorInner::Guest("unresolved dependency".into())))
        });
        failing.expect_must_get_valid_record().times(0);
        set_hdi(failing);
        assert!(resolve_link_delete_target(action_hash(7)).is_err());

        let delete = DeleteLink {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(2),
            action_seq: 2,
            prev_action: action_hash(4),
            base_address: action_hash(5).into(),
            link_add_address: action_hash(6),
        };
        let mut invalid = MockHdi::new();
        invalid
            .expect_must_get_action()
            .times(1)
            .return_once(move |_| Ok(signed_action(Action::DeleteLink(delete))));
        invalid.expect_must_get_valid_record().times(0);
        set_hdi(invalid);

        let violation = resolve_link_delete_target(action_hash(7)).unwrap().unwrap_err();
        assert_eq!(violation.to_string(), "MAP-PVL-2004: link delete target is invalid");
    }
}
