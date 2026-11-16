//! Bounded initial-constitution admission. Observed domain state is not execution authority.
use core_types::{HolonError, HolonId};
use holons_core::{reference_layer::ReadableHolon, HolonCollectionApi};
use map_commands_runtime::Runtime;
use serde::Serialize;
use std::sync::RwLock;

/// Backend observations of the saved Space. An empty current collection cannot
/// certify authoritative absence across lineage history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SovereigntyObservation {
    NoAssociationObserved,
    AssociationPresent,
    Ambiguous,
}

/// Host-owned target resolved through the configured runtime, never from IPC identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConstitutionTarget {
    pub(crate) space: HolonId,
    pub(crate) sovereignty: SovereigntyObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AdmissionBlocker {
    StartupIncomplete,
    LifecycleEvidenceUnavailable,
    AssociationAlreadyEstablished,
    EligibilityUnverified,
    CredentialCustodyUnverified,
    AcquisitionEnforcementUnavailable,
    RestartAuthorizationUnavailable,
}

/// No credential is issued here. The existing backend signer is a technical
/// author, not an authorization grant or a sovereign principal. Until custody,
/// historical eligibility and complete acquisition enforcement are delivered,
/// every request remains denied even from the approved Welcome page.
pub(crate) fn blockers(target: &ConstitutionTarget) -> Vec<AdmissionBlocker> {
    match target.sovereignty {
        SovereigntyObservation::NoAssociationObserved => vec![
            AdmissionBlocker::EligibilityUnverified,
            AdmissionBlocker::CredentialCustodyUnverified,
            AdmissionBlocker::AcquisitionEnforcementUnavailable,
        ],
        SovereigntyObservation::AssociationPresent => vec![
            AdmissionBlocker::AssociationAlreadyEstablished,
            AdmissionBlocker::RestartAuthorizationUnavailable,
        ],
        SovereigntyObservation::Ambiguous => vec![AdmissionBlocker::LifecycleEvidenceUnavailable],
    }
}

/// Presentation evidence only; it never grants authority, survives a restart,
/// or records acquisition completion. Persisted relationships remain authoritative.
#[derive(Debug, Default)]
pub(crate) struct WelcomeLifecycle {
    observation: RwLock<Option<Result<ConstitutionTarget, ()>>>,
}

impl WelcomeLifecycle {
    pub(crate) fn record(&self, observation: Result<ConstitutionTarget, ()>) -> Result<(), String> {
        *self.observation.write().map_err(|e| e.to_string())? = Some(observation);
        Ok(())
    }

    pub(crate) fn observation(&self) -> Result<Option<Result<ConstitutionTarget, ()>>, String> {
        Ok(self.observation.read().map_err(|e| e.to_string())?.clone())
    }
}

/// Read saved evidence through the existing transaction/reference surface. This
/// does not enable bootstrap, write a marker, acquire a Space or replay imports.
pub(crate) async fn inspect_target(runtime: &Runtime) -> Result<ConstitutionTarget, HolonError> {
    let tx = runtime.session().begin_transaction().await?;
    let observed = (|| {
        let context = runtime.session().get_transaction(&tx)?;
        let space = context.discover_local_holon_space()?.ok_or_else(|| {
            HolonError::InvalidState(
                "No persisted local Space is available for constitution".into(),
            )
        })?;
        // Verify the supported semantic foundation exists on Open without replaying imports.
        for key in ["MAP Agent Spaces Schema-v0.1.0", "PersonAgent.BioSocialAgent"] {
            context.lookup().get_saved_holon_by_key(&key.into())?;
        }
        let related = space.related_holons("IsInteriorOf")?;
        let count = related
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .get_count()
            .0;
        let sovereignty = match count {
            0 => SovereigntyObservation::NoAssociationObserved,
            1 => SovereigntyObservation::AssociationPresent,
            _ => SovereigntyObservation::Ambiguous,
        };
        Ok(ConstitutionTarget { space: space.holon_id()?, sovereignty })
    })();
    let archived = runtime.session().archive_transaction(&tx);
    let target = observed?;
    archived?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::LocalId;

    #[test]
    fn observation_does_not_issue_authority_or_reopen_acquisition() {
        for (sovereignty, expected) in [
            (
                SovereigntyObservation::NoAssociationObserved,
                AdmissionBlocker::EligibilityUnverified,
            ),
            (
                SovereigntyObservation::AssociationPresent,
                AdmissionBlocker::AssociationAlreadyEstablished,
            ),
            (SovereigntyObservation::Ambiguous, AdmissionBlocker::LifecycleEvidenceUnavailable),
        ] {
            let target =
                ConstitutionTarget { space: HolonId::Local(LocalId(vec![9; 39])), sovereignty };
            let denied = blockers(&target);
            assert!(denied.contains(&expected));
            assert!(!denied.is_empty());
        }
    }

    #[test]
    fn failed_observation_replaces_previous_display_evidence_and_restart_clears_it() {
        let lifecycle = WelcomeLifecycle::default();
        lifecycle
            .record(Ok(ConstitutionTarget {
                space: HolonId::Local(LocalId(vec![9; 39])),
                sovereignty: SovereigntyObservation::NoAssociationObserved,
            }))
            .unwrap();
        lifecycle.record(Err(())).unwrap();
        assert_eq!(lifecycle.observation().unwrap(), Some(Err(())));
        assert_eq!(WelcomeLifecycle::default().observation().unwrap(), None);
    }
}
