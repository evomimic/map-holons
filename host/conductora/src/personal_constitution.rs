//! PersonAgent creation through the existing transaction and Commit surfaces.
//! This native executor is not an admission boundary and has no IPC registration.
use base_types::MapString;
use core_types::HolonError;
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    reference_layer::{ReadableHolon, WritableHolon},
    RelationshipReadHint,
};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::sync::Arc;

/// Personal naming data, independent of keys, principal identifiers and authority.
pub struct PersonalName {
    pub last_name: MapString,
    pub first_name: Option<MapString>,
    pub middle_name: Option<MapString>,
}

/// Stage the keyless PersonAgent and declared interior association, then Commit once.
///
/// The trusted caller owns admission and the transaction. This does not infer
/// authority from an empty inverse, protect competing acquisitions or implement
/// historical continuity constraints. The local packaged workflow supplies admission.
/// The caller retains the transaction and every ordinary Commit outcome for
/// remediation/resubmission; retries use Commit, not another PersonAgent create.
pub async fn create_personal_agent(
    runtime: &Runtime,
    context: Arc<TransactionContext>,
    names: PersonalName,
) -> Result<MapResult, HolonError> {
    if context.is_bootstrap_provisioning() || !context.staged_references()?.is_empty() {
        return Err(HolonError::InvalidState(
            "Initial PersonAgent creation requires an ordinary empty transaction; retained work must be remediated and resubmitted".into(),
        ));
    }
    let space = context.get_space_holon()?.ok_or_else(|| {
        HolonError::InvalidState(
            "PersonAgent creation requires an existing local HolonSpace".into(),
        )
    })?;
    let inverse =
        space.related_holons_with_hint("IsInteriorOf", RelationshipReadHint::RequireFresh)?;
    if !inverse
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
        .get_members()
        .is_empty()
    {
        return Err(HolonError::InvalidState(
            "A sovereign association is already recorded; do not create another PersonAgent".into(),
        ));
    }
    let descriptor =
        context.lookup().get_saved_holon_by_key(&"PersonAgent.BioSocialAgent".into())?;
    let mut transient = context.mutation().new_holon(None)?;
    transient.with_descriptor(descriptor.into())?;
    if !transient.effective_descriptor()?.effective_key_rule()?.is_keyless()? {
        return Err(HolonError::InvalidState(
            "This installation's PersonAgent descriptor is not keyless; use compatible test state or explicitly update its schema".into(),
        ));
    }
    transient.with_property_value("LastName", names.last_name)?;
    if let Some(value) = names.first_name {
        transient.with_property_value("FirstName", value)?;
    }
    if let Some(value) = names.middle_name {
        transient.with_property_value("MiddleName", value)?;
    }
    let mut person = context.mutation().stage_new_holon(transient)?;
    person.add_related_holons("HasInterior", vec![space])?;
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
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConstitutionOutcome {
    Complete,
    Rejected,
    Incomplete,
}

/// One constitution attempt per installation runtime. The conductor's existing
/// installation lease excludes a second process. This is not distributed
/// acquisition arbitration or historical sovereign-continuity enforcement.
#[derive(Default)]
pub(crate) struct WelcomeConstitution {
    retained: tokio::sync::Mutex<Option<holons_core::core_shared_objects::transactions::TxId>>,
}

impl WelcomeConstitution {
    pub(crate) fn available(&self) -> bool {
        self.retained.try_lock().is_ok_and(|tx| tx.is_none())
    }

    /// Keep non-complete work in the RuntimeSession for existing remediation;
    /// repeated Welcome requests cannot restage a second candidate.
    pub(crate) async fn submit(
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
        let target = crate::constitution_admission::inspect_target(runtime).await?;
        if target.sovereignty
            != crate::constitution_admission::SovereigntyObservation::NoAssociationObserved
        {
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
                .property_value("CommitRequestStatus")?
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
