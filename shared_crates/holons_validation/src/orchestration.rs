use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::Arc,
};

use core_types::{
    CommitValidationViolation, CommitValidationViolationKind, HolonError, ValidationSubjectPath,
};
use holons_core::core_shared_objects::{holon::ValidationState, transactions::TransactionContext};
use holons_core::{Descriptor, HolonDescriptor, HolonReference, ReadableHolon, StagedReference};

use crate::handlers::finding;
use crate::validators::{resolve_holon_descriptor, validate_described_holon};

use crate::{
    CommitValidationReport, HolonValidationContext, HolonValidationSubject, ValidationCollector,
};

/// Builds a prospective replacement index before assessment and installs replacement
/// outcomes only after the whole assessment completes. Competition rejects uniformly
/// across live holons; independent checks continue. The C2 readiness entry point adds
/// bounded Schema scopes at the Phase 9 activation boundary, routing their findings
/// by primary subject without staging unchanged Schemas.
///
/// The caller derives candidates from the complete Nursery using
/// `StagedReference::is_live_validation_candidate`. All candidates must belong to the
/// supplied transaction and remain unchanged throughout assessment and outcome installation.
/// Previous validation states never exclude a candidate from reassessment.
/// A supplied abandoned or committed entry is a caller error, detected before any
/// outcomes are installed.
/// Populated authored relationships must be licensed by the completed source
/// contract. These findings join conformance findings in the existing rejection
/// gate, before Commit writes any candidate node or relationship.
///
/// An assessment error leaves all prior outcomes untouched. Completed assessment replaces
/// state and findings together per candidate, preserving operational errors; installation
/// does not provide transaction-wide atomic mutation. An empty set requires no schema anchors.
pub fn validate_commit_candidates(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
) -> Result<CommitValidationReport, HolonError> {
    if candidates.is_empty() {
        return Ok(CommitValidationReport::default());
    }

    check_candidates(candidates)?;
    // Identity grouping precedes anchor lookup: competing updates to a rule anchor
    // must reject semantically, not escape as a duplicate-key lookup error.
    let reader = holons_core::ProspectiveDescriptorReader::new(context, candidates)?;
    let competition = crate::competing_replacement_findings(&reader);
    if !competition.is_empty() {
        return assess_competition(context, candidates, &reader, competition);
    }
    let validation_context = HolonValidationContext::resolve(context)?;
    let mut assessment = PreparedAssessment::default();
    for candidate in candidates {
        if !candidate.is_live_validation_candidate()? {
            return Err(HolonError::InvalidParameter(format!(
                "Commit validation requires a live staged candidate: {}",
                candidate.reference_id_string()
            )));
        }
        let holon = HolonReference::from(candidate);
        let mut collector = ValidationCollector::default();
        let subject = HolonValidationSubject { holon: &holon };
        if let Some(descriptor) = resolve_holon_descriptor(subject, &mut collector)? {
            validate_authored_relationships(candidate, &descriptor, &mut collector)?;
            validate_described_holon(subject, &descriptor, &validation_context, &mut collector)?;
        }
        // Keep the candidate association directly; finding identities are diagnostics,
        // not lookup keys for installing staged outcomes.
        assessment.record_candidate(candidate, collector.into_report());
    }

    assessment.install_outcomes()
}

/// Keeps installable candidate ranges separate from the complete, flat report.
/// Aggregate findings can be appended to the report without inventing a staged carrier.
#[derive(Default)]
pub(crate) struct PreparedAssessment<'a> {
    report: CommitValidationReport,
    candidate_findings: Vec<(&'a StagedReference, Range<usize>)>,
}

impl<'a> PreparedAssessment<'a> {
    /// Partition a multi-subject scope before creating installable ranges. Only exact
    /// live staged subject identities are carriers; all other findings stay unattached.
    pub(crate) fn from_scope(
        candidates: &'a [StagedReference],
        report: CommitValidationReport,
    ) -> Result<Self, HolonError> {
        check_candidates(candidates)?;
        // Diagnostic strings route findings; candidate distinctness uses temporary IDs
        // in the shared input check above.
        let indices: HashMap<_, _> = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| (candidate.reference_id_string(), index))
            .collect();
        let mut groups = vec![Vec::new(); candidates.len()];
        let mut assessment = Self::default();
        for finding in report.violations {
            if let Some(index) =
                subject_identity(&finding.subject).and_then(|identity| indices.get(identity))
            {
                groups[*index].push(finding);
            } else {
                assessment.push_aggregate(finding);
            }
        }
        for (candidate, findings) in candidates.iter().zip(groups) {
            assessment
                .record_candidate(candidate, CommitValidationReport::from_candidate(findings));
        }
        Ok(assessment)
    }

    /// Append without shifting existing candidate ranges. Report accumulation must remain
    /// append-only until installation; aggregate findings have no staged outcome association.
    pub(crate) fn push_aggregate(&mut self, finding: CommitValidationViolation) {
        self.report.unattached_indices.push(self.report.violations.len());
        self.report.violations.push(finding);
    }

    pub(crate) fn record_candidate(
        &mut self,
        candidate: &'a StagedReference,
        report: CommitValidationReport,
    ) {
        let start = self.report.violation_count();
        self.report.violations.extend(report.violations);
        // Keep the reference association: diagnostic identities are not installation keys.
        self.candidate_findings.push((candidate, start..self.report.violation_count()));
    }

    /// Called only after every assessment succeeds; errors before this leave outcomes untouched.
    pub(crate) fn install_outcomes(self) -> Result<CommitValidationReport, HolonError> {
        let Self { report, candidate_findings } = self;
        for (candidate, range) in candidate_findings {
            let findings = report.violations[range].to_vec();
            let state = if findings.is_empty() {
                ValidationState::Validated
            } else if findings
                .iter()
                .any(|finding| matches!(finding.kind, CommitValidationViolationKind::NoDescriptor))
            {
                ValidationState::NoDescriptor
            } else {
                ValidationState::Invalid
            };
            candidate.replace_validation_outcome(state, findings)?;
        }
        Ok(report)
    }
}

/// Checks completed authored state, not a saved holon's navigation surface.
/// The same declared-only contract used by mutation policy licenses these names;
/// no target descriptor or materialized inverse index participates.
fn validate_authored_relationships(
    candidate: &StagedReference,
    descriptor: &HolonDescriptor,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let declared_names = descriptor
        .effective_declared_relationships()?
        .into_iter()
        .map(|declaration| declaration.base_relationship_name().map(|name| name.to_string()))
        .collect::<Result<HashSet<_>, HolonError>>()?;
    let mut relationships = candidate.all_related_holons()?.iter();
    relationships.sort_by_key(|(name, _)| name.to_string());
    for (name, collection) in relationships {
        let members = collection
            .read()
            .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
            .get_members()
            .to_vec();
        if members.is_empty() || declared_names.contains(&name.to_string()) {
            continue;
        }
        for target in members {
            finding(
                collector,
                CommitValidationViolationKind::RuleViolation { code: "UndeclaredRelationship".into() },
                None,
                &ValidationSubjectPath::Relationship {
                    source_identity: candidate.reference_id_string(),
                    name: name.to_string(),
                    target_identity: target.reference_id_string(),
                },
                Some(descriptor.holon().reference_id_string()),
                format!("Populated relationship {name} must be declared by the source's effective descriptor; remove it or author a declared forward relationship."),
            );
        }
    }
    Ok(())
}

/// The structured primary subject determines the staged carrier, never collector order.
pub(crate) fn subject_identity(subject: &ValidationSubjectPath) -> Option<&str> {
    match subject {
        ValidationSubjectPath::Holon { holon_identity }
        | ValidationSubjectPath::Property { holon_identity, .. }
        | ValidationSubjectPath::Value { holon_identity, .. } => Some(holon_identity),
        ValidationSubjectPath::Relationship { source_identity, .. } => Some(source_identity),
        ValidationSubjectPath::Transaction => None,
    }
}

pub(crate) fn check_candidates(candidates: &[StagedReference]) -> Result<(), HolonError> {
    let mut seen = HashSet::new();
    for candidate in candidates {
        if !candidate.is_live_validation_candidate()? || !seen.insert(candidate.temporary_id()) {
            return Err(HolonError::InvalidParameter(format!(
                "Commit validation requires distinct live candidates: {}",
                candidate.reference_id_string()
            )));
        }
    }
    Ok(())
}

/// Competition is schema-independent and activates in Phase 8. Continue independent C1
/// checks, but never read a contested definition through the legacy current-view path.
fn assess_competition(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
    reader: &holons_core::ProspectiveDescriptorReader,
    competition: Vec<CommitValidationViolation>,
) -> Result<CommitValidationReport, HolonError> {
    use crate::assessment_support::{blocked, recover, recover_transaction};
    let mut collector = ValidationCollector::default();
    for finding in competition {
        collector.record(finding);
    }
    let values = recover_transaction(
        crate::ValueValidationContext::resolve_in_view(context, reader),
        &mut collector,
    )?;
    let universal = recover_transaction(
        holons_core::UniversalDescriptorContract::resolve_with_reader(context, reader),
        &mut collector,
    )?;
    for candidate in candidates {
        let subject = HolonReference::from(candidate);
        if !crate::readiness::contract_content_available(context, &subject, reader, &mut collector)?
        {
            continue;
        }
        let Some(descriptor) =
            resolve_holon_descriptor(HolonValidationSubject { holon: &subject }, &mut collector)?
        else {
            continue;
        };
        let (Some(values), Some(universal)) = (&values, &universal) else {
            blocked(
                &mut collector,
                &subject,
                "A required validation anchor has contested content.".into(),
            );
            continue;
        };
        let result = crate::prospective_validation::prepare_bindings(
            descriptor.holon(),
            crate::contexts::SubjectLevel::Holon,
            values,
            reader,
            &crate::assessment_support::path(&subject),
            &mut collector,
        );
        if let Some(bindings) = recover(result, &subject, &mut collector)? {
            let result = crate::prospective_validation::assess_subject(
                &subject,
                descriptor.holon(),
                &bindings,
                values,
                universal,
                reader,
                &mut collector,
            );
            recover(result, &subject, &mut collector)?;
        }
    }
    PreparedAssessment::from_scope(candidates, collector.into_report())?.install_outcomes()
}
