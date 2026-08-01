//! Holochain-aware validation for agent-activity operations.
//!
//! This adapter owns predecessor resolution and joining policy so the Integrity zome callback
//! remains limited to flattened-operation dispatch and callback-result projection. These rules are
//! fixed Holochain integrity behavior, not descriptor-independent PVL.

use std::fmt;

use hdi::prelude::*;

/// A completed, deterministic rejection of agent-activity policy.
///
/// This type deliberately has no MAP-PVL code because agent joining is outside PVL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentActivityRejection {
    CreateAgentPredecessorMustBeAgentValidationPkg,
}

impl fmt::Display for AgentActivityRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateAgentPredecessorMustBeAgentValidationPkg => formatter.write_str(
                "The previous action for a `CreateAgent` action must be an `AgentValidationPkg`",
            ),
        }
    }
}

/// Resolves and validates the single predecessor required by a `CreateAgent` operation.
///
/// Dependency unavailability remains an outer `ExternResult` error so Holochain can defer
/// validation instead of turning a temporarily missing predecessor into permanent invalidity.
pub fn validate_create_agent(
    agent: AgentPubKey,
    action: &Create,
) -> ExternResult<Result<(), AgentActivityRejection>> {
    let previous_action = must_get_action(action.prev_action.clone())?;
    let Action::AgentValidationPkg(AgentValidationPkg { membrane_proof, .. }) =
        previous_action.action()
    else {
        return Ok(Err(AgentActivityRejection::CreateAgentPredecessorMustBeAgentValidationPkg));
    };

    validate_agent_joining(agent, membrane_proof)
}

/// Applies MAP's joining policy to the proof supplied by the validation package.
///
/// The current DNA is open-membership, but retaining this explicit seam keeps proof policy beside
/// the Holochain action extraction that supplies it.
fn validate_agent_joining(
    _agent_pub_key: AgentPubKey,
    _membrane_proof: &Option<MembraneProof>,
) -> ExternResult<Result<(), AgentActivityRejection>> {
    Ok(Ok(()))
}

#[cfg(test)]
mod tests {
    use mockall::mock;

    use super::*;

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

    fn agent(seed: u8) -> AgentPubKey {
        AgentPubKey::from_raw_36(vec![seed; 36])
    }

    fn create_agent() -> Create {
        Create {
            author: agent(1),
            timestamp: Timestamp::from_micros(2),
            action_seq: 2,
            prev_action: action_hash(3),
            entry_type: EntryType::AgentPubKey,
            entry_hash: EntryHash::from_raw_36(vec![5; 36]),
            weight: EntryRateWeight::default(),
        }
    }

    fn signed_action(action: Action) -> SignedActionHashed {
        SignedHashed::with_presigned(
            HoloHashed::with_pre_hashed(action, action_hash(4)),
            Signature([0; SIGNATURE_BYTES]),
        )
    }

    fn install_predecessor(action: Action) {
        let predecessor = signed_action(action);
        let mut mock = MockHdi::new();
        mock.expect_must_get_action().times(1).return_once(move |_| Ok(predecessor));
        mock.expect_must_get_valid_record().times(0);
        set_hdi(mock);
    }

    #[test]
    fn create_agent_accepts_an_agent_validation_package_predecessor() {
        install_predecessor(Action::AgentValidationPkg(AgentValidationPkg {
            author: agent(1),
            timestamp: Timestamp::from_micros(1),
            action_seq: 1,
            prev_action: action_hash(2),
            membrane_proof: None,
        }));

        assert_eq!(validate_create_agent(agent(1), &create_agent()), Ok(Ok(())));
    }

    #[test]
    fn create_agent_rejects_another_predecessor_kind_deterministically() {
        install_predecessor(Action::InitZomesComplete(InitZomesComplete {
            author: agent(1),
            timestamp: Timestamp::from_micros(1),
            action_seq: 1,
            prev_action: action_hash(2),
        }));

        assert_eq!(
            validate_create_agent(agent(1), &create_agent()),
            Ok(Err(AgentActivityRejection::CreateAgentPredecessorMustBeAgentValidationPkg))
        );
    }

    #[test]
    fn missing_predecessor_remains_an_outer_error() {
        let mut mock = MockHdi::new();
        mock.expect_must_get_action().times(1).return_once(|_| {
            Err(wasm_error!(WasmErrorInner::Guest(
                "agent predecessor dependency unavailable".into()
            )))
        });
        mock.expect_must_get_valid_record().times(0);
        set_hdi(mock);

        let error = validate_create_agent(agent(1), &create_agent())
            .expect_err("dependency failures must not become completed invalid verdicts");

        assert!(error.to_string().contains("agent predecessor dependency unavailable"));
    }
}
