//! Commit assessment of candidates through their exact bound references.
use crate::{
    assessment_support::{blocked, path, targets},
    contexts::SubjectLevel,
    handlers::finding,
    outcomes::PreparedAssessment,
    schema_rules::{self, SchemaRuleProducts},
    schema_view::{OwnedCandidate, SchemaWorkset},
    subject_assessment::{self, BindingRoleMemo, PreparedBinding},
    CommitAssessment, CommitValidationReport, ConstraintDeclarationAssessment,
    DescriptorRuleProducts, ValidationCollector, ValidationObservations, ValueValidationContext,
};
use core_types::{CommitValidationViolationKind, HolonError, HolonId, TemporaryId};
use holons_core::core_shared_objects::{holon::StagedState, Holon};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    descriptors::{resolve_describing_type, SchemaOwnershipKind},
    DefinitionIdentity, DescribingTypeResolution, DescriptorKindSemantics, HolonReference,
    StagedReference, StructuralPrerequisites, UniversalDescriptorContract,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use type_names::{CoreDefinitionRole, CoreRelationshipTypeName, CoreValidationRuleName};

/// Shares universal contracts by exact meta-type identity within this assessment.
/// Only diagnosed descriptor subjects can receive the abstract exemption.
fn prepare_universal_contracts(
    context: &Arc<TransactionContext>,
    subjects: &mut [SubjectPreparation],
    packages: &crate::descriptor_package::DescriptorPackages,
) -> Result<(), HolonError> {
    use std::collections::hash_map::Entry;

    let mut contracts = HashMap::<DefinitionIdentity, Arc<UniversalDescriptorContract>>::new();
    for subject in subjects.iter_mut().filter(|subject| subject.is_descriptor) {
        let Some(meta_type) = subject
            .prerequisites
            .governing_lineage
            .as_ref()
            .and_then(|diagnosis| diagnosis.valid_lineage())
            .and_then(|lineage| lineage.member_with_role(CoreDefinitionRole::MetaTypeDescriptor))
        else {
            continue;
        };
        let identity = DefinitionIdentity::for_reference(meta_type, context)?;
        let contract = match contracts.entry(identity) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(UniversalDescriptorContract::from_effective_schema(
                    context,
                    packages.effective_schema(meta_type)?,
                )?))
            }
        };
        subject.universal_contract = Some(Arc::clone(contract));
    }
    Ok(())
}

struct SubjectPreparation {
    key_claim: Option<crate::key_evidence::KeyClaim>,
    subject: HolonReference,
    prerequisites: StructuralPrerequisites,
    products: DescriptorRuleProducts,
    bindings: Vec<PreparedBinding>,
    is_descriptor: bool,
    universal_contract: Option<Arc<UniversalDescriptorContract>>,
    is_constraint: bool,
    is_rule: bool,
    is_schema: bool,
}

/// Assesses all candidates and installs their validation outcomes after a completed pass.
/// A scope is discarded in full on operational failure, preserving prior outcomes.
/// No unchanged Schema is staged; no saved definition is mutated. The view may include an
/// unchanged ForUpdate candidate whose eventual persistence outcome is NoAction.
pub fn validate_commit_candidates(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
) -> Result<CommitValidationReport, HolonError> {
    validate_commit_candidates_with_observer(context, candidates, |_| {})
}

/// Reports phase boundaries to optional profiling/test instrumentation. The observer
/// cannot provide schema inputs or alter the assessment's acceptance policy.
pub fn validate_commit_candidates_with_observer(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
    mut observer: impl FnMut(crate::AssessmentPhase),
) -> Result<CommitValidationReport, HolonError> {
    validate_commit_candidates_with_profile(context, candidates, |event| {
        if let crate::AssessmentEvent::Phase(phase) = event {
            observer(phase);
        }
    })
}

/// Reports detailed phases and aggregate preparation/consumption counts without a host clock.
pub fn validate_commit_candidates_with_profile(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
    observer: impl FnMut(crate::AssessmentEvent),
) -> Result<CommitValidationReport, HolonError> {
    let (assessment, _) = assess_commit_candidates_with_observer(context, candidates, observer)?;
    assessment.install_outcomes()
}

/// Assesses the prospective Commit view without installing outcomes or mutating staged holons.
/// Operational errors invalidate the entire assessment and return no partial report.
pub fn assess_commit_candidates(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
) -> Result<CommitAssessment, HolonError> {
    let (assessment, observations) =
        assess_commit_candidates_with_observer(context, candidates, |_| {})?;
    Ok(CommitAssessment { report: assessment.into_report(), observations })
}

fn assess_commit_candidates_with_observer<'a>(
    context: &Arc<TransactionContext>,
    candidates: &'a [StagedReference],
    mut observer: impl FnMut(crate::AssessmentEvent),
) -> Result<(PreparedAssessment<'a>, ValidationObservations), HolonError> {
    let _frozen_validation = context.begin_frozen_validation()?;
    observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::Construction));
    if candidates.is_empty() {
        return Ok((PreparedAssessment::default(), ValidationObservations::default()));
    }
    ensure_staged_ancestry_invariant(context, candidates)?;
    require_distinct_live_candidates(candidates)?;
    let collector = ValidationCollector::default();
    construct_assessment_with_observer(context, candidates, collector, &mut observer)?
        .assess(&mut observer)
}

/// Checks constructor-owned ancestry before assessment, including updates assembled through
/// ungoverned operations or wire reconstruction. Reads only staged memory; malformed ancestry
/// must never be repaired after assessment or replayed onto a graph-only source.
fn ensure_staged_ancestry_invariant(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
) -> Result<(), HolonError> {
    let predecessor = CoreRelationshipTypeName::Predecessor.as_relationship_name();
    for candidate in candidates {
        holons_core::assert_reference_transaction_compatible(&candidate.into(), context)?;
        let rc_holon = candidate.get_holon_to_commit(context)?;
        let holon = rc_holon.read().map_err(|error| {
            HolonError::FailedToAcquireLock(format!("Reading staged ancestry: {error}"))
        })?;
        let Holon::Staged(staged) = &*holon else {
            return Err(HolonError::InvalidType("Expected a staged candidate".into()));
        };
        if !matches!(
            staged.staged_state(),
            StagedState::ForUpdate
                | StagedState::ForUpdateGraphOnly
                | StagedState::ForUpdateNewVersion
        ) {
            continue;
        }
        let collection = staged.get_staged_relationship(&predecessor)?;
        let collection = collection.read().map_err(|error| {
            HolonError::FailedToAcquireLock(format!("Reading staged Predecessor: {error}"))
        })?;
        let members = collection.get_members();
        let source = staged.versioned_source_id_ref();
        let exact_source = match members.as_slice() {
            [HolonReference::Smart(reference)] => {
                source.is_some_and(|source| reference.holon_id() == HolonId::Local(source.clone()))
            }
            _ => false,
        };
        let touched = staged.touched_relationship_names().contains(&predecessor);
        if !exact_source || touched {
            return Err(HolonError::InvalidState(format!(
                "Candidate {} requires one untouched Predecessor naming saved source {:?}; \
                 observed Predecessor {:?}, touched={touched}",
                candidate.reference_id_string(),
                source,
                members.iter().map(HolonReference::reference_id_string).collect::<Vec<_>>(),
            )));
        }
    }
    Ok(())
}

/// Preserve Commit's carrier grouping and finding order without installing outcomes.
fn finish_assessment<'a>(
    candidates: &'a [StagedReference],
    collector: ValidationCollector,
) -> Result<(PreparedAssessment<'a>, ValidationObservations), HolonError> {
    let (report, observations) = collector.into_parts();
    Ok((PreparedAssessment::from_scope(candidates, report)?, observations))
}

/// Immutable prepared inputs and their cache leases live until assessment finishes.
pub(crate) struct ConstructedAssessment<'a, 'candidates> {
    context: &'a Arc<TransactionContext>,
    candidates: &'candidates [StagedReference],
    values: ValueValidationContext,
    prepared: Vec<SubjectPreparation>,
    binding_roles: BindingRoleMemo,
    workset: SchemaWorkset,
    packages: crate::descriptor_package::DescriptorPackages,
    collector: ValidationCollector,
}

#[cfg(test)]
pub(crate) fn construct_assessment<'a, 'candidates>(
    context: &'a Arc<TransactionContext>,
    candidates: &'candidates [StagedReference],
    collector: ValidationCollector,
) -> Result<ConstructedAssessment<'a, 'candidates>, HolonError> {
    construct_assessment_with_observer(context, candidates, collector, &mut |_| {})
}

fn construct_assessment_with_observer<'a, 'candidates>(
    context: &'a Arc<TransactionContext>,
    candidates: &'candidates [StagedReference],
    mut collector: ValidationCollector,
    observer: &mut impl FnMut(crate::AssessmentEvent),
) -> Result<ConstructedAssessment<'a, 'candidates>, HolonError> {
    observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::SubjectPreparation));
    let mut key_claims = crate::key_evidence::prepare_key_evidence(context, candidates, observer)?;
    let kinds = DescriptorKindSemantics::new(context);
    let diagnoses = Arc::new(crate::structural_diagnoses::StructuralDiagnoses::new(context));
    let mut prepared = Vec::new();
    let mut owned = Vec::new();
    let mut schemas = Vec::new();
    // Structural/kind products belong to definitions; each candidate still owns its conformance.
    let mut definition_products =
        HashMap::<DefinitionIdentity, (StructuralPrerequisites, DescriptorRuleProducts)>::new();
    // Structural diagnosis always precedes classification and binding discovery.
    for candidate in candidates {
        let subject = HolonReference::from(candidate);
        let identity = DefinitionIdentity::for_reference(&subject, context)?;
        let (prerequisites, products) = if let Some(products) = definition_products.get(&identity) {
            products.clone()
        } else {
            let definition = identity.definition_reference(context, &subject);
            let subject_lineage = crate::assessment_phase::observe_operation(
                observer,
                "StructuralDiagnosis",
                "PreparedDefinition",
                || diagnoses.get(&definition).map(|diagnosis| (*diagnosis).clone()),
            )?;
            let describing_type = resolve_describing_type(&definition)?;
            let governing_lineage =
                if let DescribingTypeResolution::Unique(descriptor) = &describing_type {
                    if holons_core::same_definition(descriptor, &definition)? {
                        Some(subject_lineage.clone())
                    } else {
                        Some(crate::assessment_phase::observe_operation(
                            observer,
                            "StructuralDiagnosis",
                            "PreparedDefinition",
                            || diagnoses.get(descriptor).map(|diagnosis| (*diagnosis).clone()),
                        )?)
                    }
                } else {
                    None
                };
            let prerequisites =
                StructuralPrerequisites { describing_type, subject_lineage, governing_lineage };
            let mut products = DescriptorRuleProducts::new(Arc::clone(&diagnoses));
            products.prepare_structure(&prerequisites);
            crate::assessment_phase::observe_operation(
                observer,
                "KindClassification",
                "PreparedDefinition",
                || products.prepare_kind(&prerequisites, &kinds),
            )?;
            definition_products.insert(identity, (prerequisites.clone(), products.clone()));
            (prerequisites, products)
        };
        match &prerequisites.describing_type {
            DescribingTypeResolution::Missing => finding(
                &mut collector,
                CommitValidationViolationKind::NoDescriptor,
                None,
                &path(&subject),
                None,
                "MissingDescribedBy: supply one direct describing type.".into(),
            ),
            DescribingTypeResolution::Multiple(targets) => finding(
                &mut collector,
                CommitValidationViolationKind::NoDescriptor,
                None,
                &path(&subject),
                None,
                format!(
                    "MultipleDescribedBy: found {} direct describing types; retain one.",
                    targets.len()
                ),
            ),
            DescribingTypeResolution::Unique(_) => {}
        }
        let is_descriptor = if let Some(lineage) = prerequisites.subject_lineage.valid_lineage() {
            kinds.is_descriptor(lineage)?
        } else {
            false
        };
        let governing =
            prerequisites.governing_lineage.as_ref().and_then(|lineage| lineage.valid_lineage());
        let is_schema =
            governing.is_some_and(|lineage| lineage.contains_role(CoreDefinitionRole::Schema));
        let is_constraint = governing
            .is_some_and(|lineage| lineage.contains_role(CoreDefinitionRole::ConstraintType));
        let is_rule =
            governing.is_some_and(|lineage| lineage.contains_role(CoreDefinitionRole::Rule));
        if is_schema {
            schemas.push(subject.clone());
        }
        for (kind, required) in [
            (SchemaOwnershipKind::Component, is_descriptor),
            (SchemaOwnershipKind::Rule, is_rule || is_constraint),
        ] {
            let edge = crate::schema_view::ownership_edge(kind);
            if required || !targets(&subject, edge)?.is_empty() {
                owned.push(OwnedCandidate { subject: subject.clone(), kind });
            }
        }
        prepared.push(SubjectPreparation {
            key_claim: key_claims.remove(&candidate.temporary_id()),
            subject,
            prerequisites,
            products,
            bindings: Vec::new(),
            is_descriptor,
            universal_contract: None,
            is_constraint,
            is_rule,
            is_schema,
        });
    }
    observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::SchemaWorksetPreparation));
    let workset = crate::assessment_phase::observe_operation(
        observer,
        "SchemaWorkset",
        "Assessment",
        || SchemaWorkset::prepare(context, &schemas, &owned, &mut collector),
    )?;
    let package_roots: Vec<_> = prepared
        .iter()
        .filter_map(|subject| match &subject.prerequisites.describing_type {
            DescribingTypeResolution::Unique(root) => Some(root.clone()),
            _ => None,
        })
        .collect();
    let package_subjects: Vec<_> = prepared.iter().map(|subject| subject.subject.clone()).collect();
    let packages = crate::descriptor_package::DescriptorPackages::construct_with_observer(
        context,
        &package_roots,
        &package_subjects,
        &workset,
        diagnoses,
        observer,
    )?;
    crate::assessment_phase::observe_operation(
        observer,
        "GraphOnlyClassification",
        "Assessment",
        || {
            crate::graph_only_deltas::classify_graph_only_delta_overlaps(
                context,
                candidates,
                Some(&packages),
                &mut collector,
            )
        },
    )?;
    let binding_contributions = packages.binding_contributions();
    let stored_diagnoses = packages.diagnoses.snapshot()?;
    let binding_roles =
        crate::assessment_phase::observe_operation(observer, "BindingRoles", "Assessment", || {
            BindingRoleMemo::prepare(
                context,
                &binding_contributions,
                stored_diagnoses.iter().filter_map(|diagnosis| diagnosis.valid_lineage()),
            )
        })?;
    crate::assessment_phase::observe_operation(
        observer,
        "UniversalContracts",
        "Assessment",
        || prepare_universal_contracts(context, &mut prepared, &packages),
    )?;
    Ok(ConstructedAssessment {
        context,
        candidates,
        values: ValueValidationContext::new(context),
        prepared,
        binding_roles,
        workset,
        packages,
        collector,
    })
}

impl<'candidates> ConstructedAssessment<'_, 'candidates> {
    /// Consumes prepared packages and records findings without installing outcomes.
    pub(crate) fn assess(
        self,
        mut observer: impl FnMut(crate::AssessmentEvent),
    ) -> Result<(PreparedAssessment<'candidates>, ValidationObservations), HolonError> {
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::PackageValidation));
        let Self {
            context,
            candidates,
            values,
            mut prepared,
            binding_roles,
            workset,
            packages,
            mut collector,
        } = self;
        let kinds = DescriptorKindSemantics::new(context);
        // Outcomes and scheduling retain every candidate, including saved-definition aliases.
        let prepared_index: HashMap<TemporaryId, usize> = prepared
            .iter()
            .enumerate()
            .map(|(index, subject)| {
                let HolonReference::Staged(candidate) = &subject.subject else {
                    unreachable!("prepared Commit subject is staged")
                };
                (candidate.temporary_id(), index)
            })
            .collect();
        // Immutable product findings fan out to all candidates sharing a definition.
        let mut definition_candidates = HashMap::<DefinitionIdentity, Vec<usize>>::new();
        for candidate in candidates {
            definition_candidates
                .entry(DefinitionIdentity::for_reference(&candidate.into(), context)?)
                .or_default()
                .push(prepared_index[&candidate.temporary_id()]);
        }
        // Assess exact defined and governing contracts before candidate conformance.
        // Saved supporting definitions supply defects and usability facts without becoming
        // full validation subjects; candidate findings remain independently routed.
        let mut interpretations = HashMap::new();
        // Effective contracts are checked once per exact definition.
        let operation = crate::assessment_phase::observe_operation_scope(
            &mut observer,
            "ContractChecks",
            "Assessment",
        );
        let mut checked_contracts = HashSet::new();
        for index in 0..prepared.len() {
            let mut contracts = Vec::new();
            if prepared[index].is_descriptor {
                contracts.push(prepared[index].subject.clone());
            }
            if let DescribingTypeResolution::Unique(descriptor) =
                &prepared[index].prerequisites.describing_type
            {
                contracts.push(descriptor.clone());
            }
            for descriptor in contracts {
                let identity = DefinitionIdentity::for_reference(&descriptor, context)?;
                if !checked_contracts.insert(identity.clone()) {
                    continue;
                }
                let descriptor = identity.definition_reference(context, &descriptor);
                let mut products = DescriptorRuleProducts::new(Arc::clone(&packages.diagnoses));
                let diagnosis = packages.diagnoses.get(&descriptor)?;
                if diagnosis.valid_lineage().is_none() {
                    continue;
                }
                let projection = packages.effective_schema(&descriptor)?;
                let interpretation = products.prepare_effective_schema(projection, &kinds)?;
                interpretations.insert(identity.clone(), interpretation);
                if let Some(indices) = definition_candidates.get(&identity) {
                    for index in indices {
                        prepared[*index].products.append(products.clone());
                    }
                } else if products.has_findings() {
                    blocked(&mut collector, &descriptor, "Saved governing contract has invalid effective member definitions; correct its descriptor commitments.".into());
                }
            }
        }
        drop(operation);
        let operation = crate::assessment_phase::observe_operation_scope(
            &mut observer,
            "BindingDispatch",
            "Assessment",
        );
        for subject in &mut prepared {
            if let Some(governing) = subject
                .prerequisites
                .governing_lineage
                .as_ref()
                .and_then(|lineage| lineage.valid_lineage())
            {
                let result = subject_assessment::prepare_bindings(
                    governing.subject(),
                    SubjectLevel::Holon,
                    &values,
                    &binding_roles,
                    &path(&subject.subject),
                    &mut collector,
                    &packages,
                );
                subject.bindings = result?;
            }
        }
        drop(operation);
        let operation = crate::assessment_phase::observe_operation_scope(
            &mut observer,
            "SchemaChecks",
            "Assessment",
        );
        let cycles = schema_rules::dependency_cycles(context, &workset.schemas)?;
        for view in &workset.schemas {
            collector.observations.schema_assessment_count += 1;
            let mut products = SchemaRuleProducts::default();
            let id = DefinitionIdentity::for_reference(&view.schema, context)?;
            if let Some(Some(witness)) = cycles.get(&id) {
                products.record(CoreValidationRuleName::SchemaDependenciesAcyclic, format!("Schema {} reaches a versioned DependsOn cycle containing {}; remove the cyclic dependency.", view.schema.reference_id_string(), witness.reference_id_string()));
            }
            if let Some(indices) = definition_candidates.get(&id) {
                for index in indices {
                    subject_assessment::dispatch_schema(
                        &prepared[*index].bindings,
                        &products,
                        &prepared[*index].subject,
                        &mut collector,
                    )?;
                }
            } else if let DescribingTypeResolution::Unique(descriptor) =
                resolve_describing_type(&view.schema)?
            {
                let result = subject_assessment::prepare_bindings(
                    &descriptor,
                    SubjectLevel::Holon,
                    &values,
                    &binding_roles,
                    &path(&view.schema),
                    &mut collector,
                    &packages,
                );
                let bindings = result?;
                subject_assessment::dispatch_schema(
                    &bindings,
                    &products,
                    &view.schema,
                    &mut collector,
                )?;
            }
        }
        drop(operation);
        // Only staged members are declaration subjects, grouped by exact Version-bound owner;
        // saved unstaged members are not reread.
        // Declaration coverage belongs to exact definitions.
        let operation = crate::assessment_phase::observe_operation_scope(
            &mut observer,
            "DeclarationChecks",
            "Assessment",
        );
        let mut declaration_members = HashSet::new();
        let declaration_start = collector.violations().len();
        for view in &workset.schemas {
            let mut declarations =
                ConstraintDeclarationAssessment::new(Arc::clone(&packages.diagnoses));
            for descriptor in &view.staged_components {
                declaration_members.insert(DefinitionIdentity::for_reference(descriptor, context)?);
                let diagnosis = packages.diagnoses.get(descriptor)?;
                let Some(lineage) = diagnosis.valid_lineage() else {
                    continue;
                };
                let mut products = DescriptorRuleProducts::new(Arc::clone(&packages.diagnoses));
                declarations.assess_contributions(
                    lineage,
                    &packages.constraint_contributions(descriptor)?,
                    &mut products,
                    &mut collector,
                )?;
                if let Some(indices) = definition_candidates
                    .get(&DefinitionIdentity::for_reference(descriptor, context)?)
                {
                    for index in indices {
                        prepared[*index].products.append(products.clone());
                    }
                }
            }
            for rule in &view.staged_rules {
                declaration_members.insert(DefinitionIdentity::for_reference(rule, context)?);
                assess_configured_rule(
                    rule,
                    &mut declarations,
                    &mut collector,
                    &packages.diagnoses,
                )?;
            }
        }
        // Invalid/missing ownership cannot remove a staged commitment from declaration assessment.
        let mut orphan_declarations =
            ConstraintDeclarationAssessment::new(Arc::clone(&packages.diagnoses));
        for subject in &mut prepared {
            if !declaration_members
                .contains(&DefinitionIdentity::for_reference(&subject.subject, context)?)
            {
                if subject.is_descriptor {
                    if let Some(lineage) = subject.prerequisites.subject_lineage.valid_lineage() {
                        orphan_declarations.assess_contributions(
                            lineage,
                            &packages.constraint_contributions(&subject.subject)?,
                            &mut subject.products,
                            &mut collector,
                        )?;
                    }
                } else if subject.is_constraint {
                    orphan_declarations.assess_constraint(&subject.subject, &mut collector)?;
                }
            }
        }
        route_definition_findings(
            context,
            &mut collector,
            declaration_start,
            &prepared,
            &definition_candidates,
        )?;
        drop(operation);
        // Supporting facts and Schema checks complete before deterministic candidate assessment.
        let is_definition = |subject: &SubjectPreparation| {
            subject.is_descriptor || subject.is_rule || subject.is_constraint || subject.is_schema
        };
        for subject in prepared.iter().filter(|subject| is_definition(subject)) {
            assess_candidate(
                context,
                subject,
                &values,
                &binding_roles,
                &packages,
                &interpretations,
                &mut collector,
                &mut observer,
            )?;
        }
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::InstanceValidation));
        for subject in prepared.iter().filter(|subject| !is_definition(subject)) {
            assess_candidate(
                context,
                subject,
                &values,
                &binding_roles,
                &packages,
                &interpretations,
                &mut collector,
                &mut observer,
            )?;
        }
        observer(crate::AssessmentEvent::Preparation(packages.preparation_metrics()));
        finish_assessment(candidates, collector)
    }
}

/// Routes each declaration finding in original collector order, expanding shared definitions
/// in candidate order and retaining every unmatched finding. Extraction and re-recording replace
/// only the completed declaration suffix; earlier findings and observations remain unchanged.
fn route_definition_findings(
    context: &Arc<TransactionContext>,
    collector: &mut ValidationCollector,
    start: usize,
    prepared: &[SubjectPreparation],
    definition_candidates: &HashMap<DefinitionIdentity, Vec<usize>>,
) -> Result<(), HolonError> {
    let mut declaration_definitions = HashMap::new();
    for subject in prepared {
        let identity = DefinitionIdentity::for_reference(&subject.subject, context)?;
        declaration_definitions
            .insert(subject.subject.full_reference_id_string(), identity.clone());
        let definition = identity.definition_reference(context, &subject.subject);
        declaration_definitions.insert(definition.full_reference_id_string(), identity);
    }
    for finding in collector.take_findings_since(start) {
        let identity = crate::outcomes::subject_identity(&finding.subject)
            .and_then(|name| declaration_definitions.get(name));
        if let Some(identity) = identity {
            for index in &definition_candidates[identity] {
                let mut routed = finding.clone();
                routed.subject = finding
                    .subject
                    .with_holon_identity(prepared[*index].subject.full_reference_id_string());
                collector.record(routed);
            }
        } else {
            collector.record(finding);
        }
    }
    Ok(())
}

/// Consumes usable interpretation facts independently of other candidates' acceptance.
fn assess_candidate(
    context: &Arc<TransactionContext>,
    subject: &SubjectPreparation,
    values: &ValueValidationContext,
    binding_roles: &BindingRoleMemo,
    packages: &crate::descriptor_package::DescriptorPackages,
    interpretations: &HashMap<DefinitionIdentity, crate::descriptor_rules::ContractInterpretation>,
    collector: &mut ValidationCollector,
    observer: &mut impl FnMut(crate::AssessmentEvent),
) -> Result<(), HolonError> {
    subject_assessment::dispatch_descriptor(
        &subject.bindings,
        &subject.products,
        &subject.subject,
        collector,
    )?;
    let inputs = crate::assessment_phase::observe_operation(
        observer,
        "InterpretationGuards",
        "Candidate",
        || {
            if let Some(governing) = subject.prerequisites.governing_lineage.as_ref() {
                let Some(lineage) = governing.valid_lineage() else {
                    blocked(
                        collector,
                        &subject.subject,
                        format!(
                            "Governing descriptor {} has a malformed Extends lineage.",
                            governing.lineage[0].reference_id_string()
                        ),
                    );
                    return Ok(None);
                };
                let package = packages.get(context, lineage.subject())?;
                let identity = DefinitionIdentity::for_reference(lineage.subject(), context)?;
                let interpretation = interpretations.get(&identity).ok_or_else(|| {
                    HolonError::InvalidState(
                        "Governing contract interpretation was not prepared".into(),
                    )
                })?;
                if !interpretation.vocabulary_usable {
                    blocked(
                        collector,
                        &subject.subject,
                        format!(
                            "Governing descriptor {} has ambiguous or unnamed effective members.",
                            package.root.reference_id_string()
                        ),
                    );
                    return Ok(None);
                }
                return Ok(Some((package, interpretation)));
            }
            Ok(None)
        },
    )?;
    if let Some((package, interpretation)) = inputs {
        subject_assessment::assess_holon(
            &subject.subject,
            &package.root,
            package.contract.as_ref().map_err(Clone::clone)?.as_ref(),
            &subject.bindings,
            subject.key_claim.as_ref(),
            values,
            binding_roles,
            subject.universal_contract.as_deref(),
            collector,
            packages,
            interpretation,
        )?;
    }
    Ok(())
}

fn assess_configured_rule(
    rule: &HolonReference,
    declarations: &mut ConstraintDeclarationAssessment,
    collector: &mut ValidationCollector,
    diagnoses: &crate::structural_diagnoses::StructuralDiagnoses,
) -> Result<(), HolonError> {
    if let DescribingTypeResolution::Unique(descriptor) = resolve_describing_type(rule)? {
        if diagnoses
            .get(&descriptor)?
            .valid_lineage()
            .is_some_and(|lineage| lineage.contains_role(CoreDefinitionRole::ConstraintType))
        {
            declarations.assess_constraint(rule, collector)?;
        }
    }
    Ok(())
}

/// Require live, distinct candidates before constructing an assessment or installing outcomes.
pub(crate) fn require_distinct_live_candidates(
    candidates: &[StagedReference],
) -> Result<(), HolonError> {
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
