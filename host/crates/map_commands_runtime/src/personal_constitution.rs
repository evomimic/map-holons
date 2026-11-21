//! Host transaction orchestration for personal constitution; callers supply admission.
use crate::{ExecutionPolicy, Runtime};
use core_types::{HolonError, HolonId};
pub use holons_core::agent_spaces::PersonalName;
use holons_core::{
    agent_spaces::{current_personal_space, sovereign_agent, stage_personal_agent, SovereignAgent},
    core_shared_objects::transactions::TransactionContext,
    ReadableHolon, SmartReference, WritableHolon,
};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use std::sync::Arc;

/// Backend observations of the saved Space. An empty current collection cannot
/// certify authoritative absence across lineage history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SovereigntyObservation {
    NoAssociationObserved,
    AssociationPresent,
    Ambiguous,
}

/// Host-owned target resolved through the configured runtime, never from IPC identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstitutionTarget {
    pub space: HolonId,
    pub sovereignty: SovereigntyObservation,
}

/// Observe saved sovereign membership in a short-lived transaction, reusing adopted identity.
/// No foundation-package lookup or import participates in a status read.
pub async fn inspect_target(runtime: &Runtime) -> Result<ConstitutionTarget, HolonError> {
    let tx = runtime.session().begin_transaction().await?;
    let observed = (|| {
        let context = runtime.session().get_transaction(&tx)?;
        let space = current_personal_space(&context)?;
        let space_id = space.holon_id()?;
        let sovereignty = match sovereign_agent(&space)? {
            SovereignAgent::Unclaimed => SovereigntyObservation::NoAssociationObserved,
            SovereignAgent::Ambiguous => SovereigntyObservation::Ambiguous,
            SovereignAgent::Agent(agent) => {
                tracing::info!(space = ?space_id, person = ?agent.holon_id()?, "Reopened recorded sovereign association");
                SovereigntyObservation::AssociationPresent
            }
        };
        Ok(ConstitutionTarget { space: space_id, sovereignty })
    })();
    let archived = runtime.session().archive_transaction(&tx);
    let target = observed?;
    archived?;
    Ok(target)
}

/// Stage the Agent, personal Space name and offerings in one Commit.
/// The caller retains non-complete work through the existing command lifecycle.
pub async fn create_personal_agent(
    runtime: &Runtime,
    context: Arc<TransactionContext>,
    names: PersonalName,
) -> Result<MapResult, HolonError> {
    let previous_space = current_personal_space(&context)?;
    let space_name = names.space_name()?;
    let mut person = stage_personal_agent(&context, names)?;
    let mut named_space = context.mutation().stage_new_version(SmartReference::new_from_id(
        context.space_read_handle(),
        previous_space.holon_id()?,
    ))?;
    named_space.with_property_value(type_names::CorePropertyTypeName::Key, space_name.clone())?;
    named_space.with_property_value(
        type_names::CorePropertyTypeName::HolonSpaceName,
        space_name.clone(),
    )?;
    named_space.with_property_value(type_names::CorePropertyTypeName::DisplayName, space_name)?;
    named_space.with_property_value(
        type_names::CorePropertyTypeName::Description,
        "Your personal Space, including its MAP foundation.",
    )?;
    let space = holons_core::HolonReference::from(named_space);
    person.remove_related_holons(
        type_names::AgentSpaceRelationshipTypeName::HasInterior,
        vec![previous_space.clone()],
    )?;
    person.add_related_holons(
        type_names::AgentSpaceRelationshipTypeName::HasInterior,
        vec![space.clone()],
    )?;
    // These are version-bound incoming associations. Author the declared
    // occurrences on their owning resources; Commit materializes each inverse
    // on the named Space version while preserving the previous associations.
    for (inverse, declared) in
        [("AffordsDancer", "AffordedByHolonSpace"), ("OffersTheme", "OfferedByHolonSpace")]
    {
        let resources = previous_space
            .related_holons_with_hint(inverse, holons_core::RelationshipReadHint::RequireFresh)?;
        let resources = resources
            .read()
            .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
            .get_members()
            .clone();
        for resource in resources {
            let mut staged = context.mutation().stage_new_version(SmartReference::new_from_id(
                context.space_read_handle(),
                resource.holon_id()?,
            ))?;
            staged.add_related_holons(declared, vec![space.clone()])?;
        }
    }
    runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context,
                action: TransactionAction::Commit,
            }),
            ExecutionPolicy::default(),
        )
        .await
}

/// Ordinary Commit outcome projected without runtime handles or credentials.
#[derive(Debug, PartialEq, Eq)]
pub enum ConstitutionOutcome {
    Complete,
    Rejected,
    Incomplete,
}

/// One constitution attempt per installation runtime. The conductor's existing
/// installation lease excludes a second process. This is not distributed
/// acquisition arbitration or historical sovereign-continuity enforcement.
#[derive(Default)]
pub struct PersonalConstitution {
    retained: tokio::sync::Mutex<Option<holons_core::core_shared_objects::transactions::TxId>>,
}

impl PersonalConstitution {
    pub fn available(&self) -> bool {
        self.retained.try_lock().is_ok_and(|tx| tx.is_none())
    }

    /// Keep non-complete work in the RuntimeSession for existing remediation;
    /// repeated Welcome requests cannot restage a second candidate.
    pub async fn submit(
        &self,
        runtime: &Runtime,
        names: PersonalName,
    ) -> Result<ConstitutionOutcome, HolonError> {
        let mut retained = self.retained.try_lock().map_err(|_| {
            HolonError::InvalidState("Personal constitution is already in progress".into())
        })?;
        if retained.is_some() {
            return Err(HolonError::InvalidState(
                "The existing constitution transaction requires remediation/resubmission; no second Agent will be created".into(),
            ));
        }
        if names.last_name.0.trim().is_empty() {
            return Err(HolonError::InvalidParameter("Last name is required".into()));
        }
        let target = inspect_target(runtime).await?;
        if target.sovereignty != SovereigntyObservation::NoAssociationObserved {
            return Err(HolonError::InvalidState(
                "This space already has a sovereign association or cannot be resolved".into(),
            ));
        }
        let tx = runtime.session().begin_transaction().await?;
        *retained = Some(tx);
        let context = runtime.session().get_transaction(&tx)?;
        let result = create_personal_agent(runtime, context.clone(), names).await;
        let outcome = match result {
            Ok(MapResult::Reference(response)) => match response
                .property_value(type_names::CorePropertyTypeName::CommitRequestStatus)?
            {
                Some(base_types::BaseValue::StringValue(status)) if status.0 == "Complete" => {
                    ConstitutionOutcome::Complete
                }
                Some(base_types::BaseValue::StringValue(status)) if status.0 == "Rejected" => {
                    ConstitutionOutcome::Rejected
                }
                Some(base_types::BaseValue::StringValue(status)) if status.0 == "Incomplete" => {
                    ConstitutionOutcome::Incomplete
                }
                _ => return Err(HolonError::InvalidState(
                    "Commit did not return a recognized completion outcome; transaction retained"
                        .into(),
                )),
            },
            Ok(_) => {
                return Err(HolonError::InvalidState(
                    "Commit response unavailable; transaction retained".into(),
                ))
            }
            // An execution error may follow persistence. Retain the original
            // transaction rather than infer safe recreation from pool contents.
            Err(error) => return Err(error),
        };
        if outcome == ConstitutionOutcome::Complete {
            runtime.session().archive_transaction(&tx)?;
            *retained = None;
        }
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn status_requires_adoption_without_attempting_guest_discovery() {
        let manager = holons_client::init_client_runtime(None);
        let runtime = Runtime::new(std::sync::Arc::new(crate::RuntimeSession::new(manager, None)));
        let result = inspect_target(&runtime).await;
        assert!(
            matches!(result, Err(HolonError::InvalidState(message)) if message.contains("not adopted"))
        );
    }
}
