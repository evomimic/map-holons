//! Bounded initial-constitution admission. Observed domain state is not execution authority.
use serde::Serialize;
use std::sync::RwLock;

use map_commands_runtime::personal_constitution::{ConstitutionTarget, SovereigntyObservation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdmissionBlocker {
    StartupIncomplete,
    LifecycleEvidenceUnavailable,
    AssociationAlreadyEstablished,
    EligibilityUnverified,
    CredentialCustodyUnverified,
    AcquisitionEnforcementUnavailable,
    RestartAuthorizationUnavailable,
    ConstitutionTransactionPending,
}

/// No credential is issued here. The existing backend signer is a technical
/// author, not an authorization grant or a sovereign principal. Until custody,
/// historical eligibility and complete acquisition enforcement are delivered,
/// these blockers gate production admission. The explicit local engineering
/// workflow uses the packaged request boundary without claiming that assurance.
pub fn blockers(target: &ConstitutionTarget) -> Vec<AdmissionBlocker> {
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
pub struct WelcomeLifecycle {
    observation: RwLock<Option<Result<ConstitutionTarget, ()>>>,
}

impl WelcomeLifecycle {
    pub fn record(&self, observation: Result<ConstitutionTarget, ()>) -> Result<(), String> {
        *self.observation.write().map_err(|e| e.to_string())? = Some(observation);
        Ok(())
    }

    pub fn observation(&self) -> Result<Option<Result<ConstitutionTarget, ()>>, String> {
        Ok(self.observation.read().map_err(|e| e.to_string())?.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{HolonId, LocalId};

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
