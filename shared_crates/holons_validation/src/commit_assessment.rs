//! Commit assessment of candidates through their exact bound references.
use crate::{
    assessment_support::{blocked, path, targets},
    contexts::SubjectLevel,
    handlers::finding,
    outcomes::PreparedAssessment,
    schema_rules::{self, SchemaRuleProducts},
    schema_view::{OwnedCandidate, SchemaWorkset},
    subject_assessment::{self, PreparedBinding},
    CommitAssessment, CommitValidationReport, ConstraintDeclarationAssessment,
    ConstraintDeclarationRoots, ContractKindRoots, DescriptorRuleProducts, ValidationCollector,
    ValidationObservations, ValueValidationContext,
};
use core_types::{CommitValidationViolationKind, HolonError, HolonId, TemporaryId};
use holons_core::core_shared_objects::{holon::StagedState, Holon};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    descriptors::{resolve_describing_type, SchemaOwnershipKind},
    ContractContributions, DefinitionIdentity, DescribingTypeResolution, DescriptorKindRoots,
    HolonReference, StagedReference, StructuralPrerequisites, UniversalDescriptorContract,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use type_names::{CoreRelationshipTypeName, CoreValidationRuleName};

/// Graph roots and rule anchors shared by one Commit assessment.
pub(crate) struct ReadinessContext {
    kinds: DescriptorKindRoots,
    contracts: ContractKindRoots,
    constraints: ConstraintDeclarationRoots,
    schema: HolonReference,
    rule: HolonReference,
    values: ValueValidationContext,
    universal: UniversalDescriptorContract,
}
impl ReadinessContext {
    pub fn resolve(
        context: &Arc<TransactionContext>,
    ) -> Result<Self, crate::AnchorResolutionError> {
        let resolve = |key| crate::resolve_validation_anchor(context, key);
        let root = resolve("TypeDescriptor")?;
        let kinds = DescriptorKindRoots::from_resolved(
            context,
            root.clone(),
            resolve("HolonType.TypeDescriptor")?,
            resolve("MetaTypeDescriptor.HolonType")?,
            resolve("MetaHolonType.MetaTypeDescriptor")?,
        )?;
        let contracts = ContractKindRoots {
            property: resolve("PropertyType.TypeDescriptor")?,
            relationship: resolve("DeclaredRelationshipType.RelationshipType")?,
            value: resolve("ValueType.TypeDescriptor")?,
        };
        let constraints = ConstraintDeclarationRoots {
            type_descriptor: root,
            constraint_type: resolve("ConstraintType.HolonType")?,
            bounded: [
                resolve("StringLengthConstraint.ConstraintType")?,
                resolve("BytesLengthConstraint.ConstraintType")?,
                resolve("NumericRangeConstraint.ConstraintType")?,
                resolve("ItemCountConstraint.ConstraintType")?,
            ],
            cardinality: resolve("CardinalityConstraint.ConstraintType")?,
            unique_items: resolve("UniqueItemsConstraint.ConstraintType")?,
        };
        let values = ValueValidationContext::resolve(context)?;
        Ok(Self {
            kinds,
            contracts,
            constraints,
            schema: resolve("Schema.HolonType")?,
            rule: resolve("Rule.HolonType")?,
            values,
            universal: UniversalDescriptorContract::resolve(context)?,
        })
    }
}

struct SubjectPreparation {
    key_claim: Option<crate::key_evidence::KeyClaim>,
    subject: HolonReference,
    prerequisites: StructuralPrerequisites,
    products: DescriptorRuleProducts,
    bindings: Vec<PreparedBinding>,
    is_descriptor: bool,
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
    observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::Construction));
    if candidates.is_empty() {
        return Ok((PreparedAssessment::default(), ValidationObservations::default()));
    }
    ensure_staged_ancestry_invariant(context, candidates)?;
    require_distinct_live_candidates(candidates)?;
    let mut collector = ValidationCollector::default();
    crate::prospective::assess_required_anchor_updates(
        context,
        candidates,
        &mut collector,
        &mut observer,
    )?;
    crate::graph_only_deltas::classify_graph_only_delta_overlaps(
        context,
        candidates,
        &mut collector,
    )?;
    observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::ReadinessResolution));
    let roots = ReadinessContext::resolve(context).map_err(HolonError::from)?;
    construct_assessment_with_observer(context, candidates, &roots, collector, &mut observer)?
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
    roots: &'a ReadinessContext,
    prepared: Vec<SubjectPreparation>,
    workset: SchemaWorkset,
    packages: crate::descriptor_package::DescriptorPackages,
    collector: ValidationCollector,
}

#[cfg(test)]
pub(crate) fn construct_assessment<'a, 'candidates>(
    context: &'a Arc<TransactionContext>,
    candidates: &'candidates [StagedReference],
    roots: &'a ReadinessContext,
    collector: ValidationCollector,
) -> Result<ConstructedAssessment<'a, 'candidates>, HolonError> {
    construct_assessment_with_observer(context, candidates, roots, collector, &mut |_| {})
}

fn construct_assessment_with_observer<'a, 'candidates>(
    context: &'a Arc<TransactionContext>,
    candidates: &'candidates [StagedReference],
    roots: &'a ReadinessContext,
    mut collector: ValidationCollector,
    observer: &mut impl FnMut(crate::AssessmentEvent),
) -> Result<ConstructedAssessment<'a, 'candidates>, HolonError> {
    observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::SubjectPreparation));
    let mut key_claims = crate::key_evidence::prepare_key_evidence(context, candidates, observer)?;
    let kinds = roots.kinds.clone();
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
            let subject_lineage =
                holons_core::ExtendsLineageDiagnosis::assess(&definition, &kinds.type_descriptor)?;
            let describing_type = resolve_describing_type(&definition)?;
            let governing_lineage =
                if let DescribingTypeResolution::Unique(descriptor) = &describing_type {
                    if holons_core::same_definition(descriptor, &definition)? {
                        Some(subject_lineage.clone())
                    } else {
                        Some(holons_core::ExtendsLineageDiagnosis::assess(
                            descriptor,
                            &kinds.type_descriptor,
                        )?)
                    }
                } else {
                    None
                };
            let prerequisites =
                StructuralPrerequisites { describing_type, subject_lineage, governing_lineage };
            let mut products = DescriptorRuleProducts::default();
            products.prepare_structure(&prerequisites, &kinds.type_descriptor);
            products.prepare_kind(&prerequisites, &kinds)?;
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
            governing.map(|lineage| lineage.contains(&roots.schema)).transpose()?.unwrap_or(false);
        let is_constraint = governing
            .map(|lineage| lineage.contains(&roots.constraints.constraint_type))
            .transpose()?
            .unwrap_or(false);
        let is_rule =
            governing.map(|lineage| lineage.contains(&roots.rule)).transpose()?.unwrap_or(false);
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
            is_constraint,
            is_rule,
            is_schema,
        });
    }
    observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::SchemaWorksetPreparation));
    let workset = SchemaWorkset::prepare(context, &roots.schema, &schemas, &owned, &mut collector)?;
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
        observer,
    )?;
    Ok(ConstructedAssessment { context, candidates, roots, prepared, workset, packages, collector })
}

impl<'candidates> ConstructedAssessment<'_, 'candidates> {
    /// Consumes prepared packages and records findings without installing outcomes.
    pub(crate) fn assess(
        self,
        mut observer: impl FnMut(crate::AssessmentEvent),
    ) -> Result<(PreparedAssessment<'candidates>, ValidationObservations), HolonError> {
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::PackageValidation));
        let Self { context, candidates, roots, mut prepared, workset, mut packages, mut collector } =
            self;
        let kinds = roots.kinds.clone();
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
        // Defined contracts are prepared before any subject consumes them. Include unstaged
        // governors, whose malformed definitions must block rather than escape as a read error.
        // Only definition-scoped product failures enter this set; candidate failures are
        // classified separately by readiness observation.
        let mut invalid = HashSet::new();
        // Effective contracts are checked once per exact definition.
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
                let mut products = DescriptorRuleProducts::default();
                let diagnosis = holons_core::ExtendsLineageDiagnosis::assess(
                    &descriptor,
                    &kinds.type_descriptor,
                );
                let diagnosis = diagnosis?;
                if diagnosis.valid_lineage().is_none() {
                    invalid.insert(identity);
                    blocked(
                        &mut collector,
                        &prepared[index].subject,
                        format!(
                            "Governing descriptor {} has a malformed Extends lineage.",
                            descriptor.reference_id_string()
                        ),
                    );
                    continue;
                }
                let contributions = ContractContributions::resolve(&descriptor)?;
                products.prepare_contract(&contributions, &kinds, &roots.contracts)?;
                if products.has_findings() {
                    invalid.insert(identity.clone());
                }
                if let Some(indices) = definition_candidates.get(&identity) {
                    for index in indices {
                        prepared[*index].products.append(products.clone());
                    }
                } else if products.has_findings() {
                    blocked(&mut collector, &descriptor, "Saved governing contract has invalid effective member definitions; correct its descriptor commitments.".into());
                }
            }
        }
        // Only staged members are declaration subjects, grouped by exact Version-bound owner;
        // saved unstaged members are not reread.
        // Declaration coverage belongs to exact definitions.
        let mut declaration_members = HashSet::new();
        let declaration_start = collector.violations().len();
        for view in &workset.schemas {
            let mut declarations = ConstraintDeclarationAssessment::new(&roots.constraints)?;
            for descriptor in &view.staged_components {
                declaration_members.insert(DefinitionIdentity::for_reference(descriptor, context)?);
                let diagnosis = holons_core::ExtendsLineageDiagnosis::assess(
                    descriptor,
                    &kinds.type_descriptor,
                )?;
                let Some(lineage) = diagnosis.valid_lineage() else {
                    continue;
                };
                let mut products = DescriptorRuleProducts::default();
                declarations.assess_descriptor(lineage, &mut products, &mut collector)?;
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
                    &roots.constraints.constraint_type,
                    &mut declarations,
                    &mut collector,
                )?;
            }
        }
        // Invalid/missing ownership cannot remove a staged commitment from declaration assessment.
        let mut orphan_declarations = ConstraintDeclarationAssessment::new(&roots.constraints)?;
        for subject in &mut prepared {
            if !declaration_members
                .contains(&DefinitionIdentity::for_reference(&subject.subject, context)?)
            {
                if subject.is_descriptor {
                    if let Some(lineage) = subject.prerequisites.subject_lineage.valid_lineage() {
                        orphan_declarations.assess_descriptor(
                            lineage,
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
            &mut invalid,
        )?;
        for subject in &mut prepared {
            if subject.products.has_findings() {
                invalid.insert(DefinitionIdentity::for_reference(&subject.subject, context)?);
            }
            if let Some(governing) = subject
                .prerequisites
                .governing_lineage
                .as_ref()
                .and_then(|lineage| lineage.valid_lineage())
            {
                let result = subject_assessment::prepare_bindings(
                    governing.subject(),
                    SubjectLevel::Holon,
                    &roots.values,
                    &path(&subject.subject),
                    &mut collector,
                    &packages,
                );
                subject.bindings = result?;
            }
            subject_assessment::dispatch_descriptor(
                &subject.bindings,
                &subject.products,
                &subject.subject,
                &mut collector,
            )?;
        }
        let cycles = schema_rules::dependency_cycles(context, &workset.schemas)?;
        for view in &workset.schemas {
            collector.observations.schema_assessment_count += 1;
            let mut products = SchemaRuleProducts::default();
            let id = DefinitionIdentity::for_reference(&view.schema, context)?;
            if let Some(Some(witness)) = cycles.get(&id) {
                products.record(CoreValidationRuleName::SchemaDependenciesAcyclic, format!("Schema {} reaches a versioned DependsOn cycle containing {}; remove the cyclic dependency.", view.schema.reference_id_string(), witness.reference_id_string()));
            }
            if products.has_findings() {
                invalid.insert(id.clone());
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
                    &roots.values,
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
        // Preparation is complete before any subject consumes commitments. Readiness
        // propagation indexes references once and consumes only newly added findings.
        let (order, mut readiness) = commitment_order(context, &prepared, &workset)?;
        for identity in invalid {
            readiness.invalidate(identity);
        }
        readiness.observe(&collector);
        let is_definition = |subject: &SubjectPreparation| {
            subject.is_descriptor || subject.is_rule || subject.is_constraint || subject.is_schema
        };
        // Each strongly connected group establishes conformance before ordinary consumers run.
        for group in &order {
            for &index in group {
                let subject = &prepared[index];
                if !is_definition(subject) {
                    continue;
                }
                assess_candidate(
                    context,
                    subject,
                    roots,
                    &packages,
                    &mut readiness,
                    &mut collector,
                )?;
            }
        }
        packages.finish(&readiness.invalid);
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::InstanceValidation));
        for group in &order {
            for &index in group {
                let subject = &prepared[index];
                if is_definition(subject) {
                    continue;
                }
                assess_candidate(
                    context,
                    subject,
                    roots,
                    &packages,
                    &mut readiness,
                    &mut collector,
                )?;
            }
        }
        // Mutually describing commitments are legal. If a later conformance check
        // invalidates an earlier dependent, expose that blocking result without re-evaluation.
        for subject in &prepared {
            let id = DefinitionIdentity::for_reference(&subject.subject, context)?;
            if readiness.invalid.contains(&id)
                && !readiness.diagnosed.contains(&subject.subject.full_reference_id_string())
            {
                blocked(
                    &mut collector,
                    &subject.subject,
                    "A required prospective commitment did not establish readiness.".into(),
                );
            }
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
    invalid: &mut HashSet<DefinitionIdentity>,
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
            invalid.insert(identity.clone());
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

/// Definition groups use constructed peer contracts provisionally. Ordinary instances
/// reach this operation only after package readiness has been established.
fn assess_candidate(
    context: &Arc<TransactionContext>,
    subject: &SubjectPreparation,
    roots: &ReadinessContext,
    packages: &crate::descriptor_package::DescriptorPackages,
    readiness: &mut ReadinessPropagation,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let id = DefinitionIdentity::for_reference(&subject.subject, context)?;
    if let Some(dependency) = readiness.blocking_dependency(&id) {
        blocked(collector, &subject.subject, format!("Required prospective commitment {dependency} is invalid; correct its reported findings before retrying."));
    } else if let Some(governing) =
        subject.prerequisites.governing_lineage.as_ref().and_then(|lineage| lineage.valid_lineage())
    {
        let package = packages.get(context, governing.subject())?;
        let definition =
            subject.is_descriptor || subject.is_rule || subject.is_constraint || subject.is_schema;
        if !definition && package.state != crate::descriptor_package::PackageState::Validated {
            blocked(
                collector,
                &subject.subject,
                "Required descriptor package did not establish readiness.".into(),
            );
        } else {
            let contract = package.contract.clone()?;
            let result = subject_assessment::assess_holon(
                &subject.subject,
                &package.root,
                &contract,
                &subject.bindings,
                subject.key_claim.as_ref(),
                &roots.values,
                &roots.universal,
                collector,
                packages,
            );
            result?;
        }
    }
    readiness.observe(collector);
    Ok(())
}

fn assess_configured_rule(
    rule: &HolonReference,
    constraint_root: &HolonReference,
    declarations: &mut ConstraintDeclarationAssessment,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    if let DescribingTypeResolution::Unique(descriptor) = resolve_describing_type(rule)? {
        if holons_core::descriptors::equals_or_extends(&descriptor, constraint_root)? {
            declarations.assess_constraint(rule, collector)?;
        }
    }
    Ok(())
}

type Dependencies = HashMap<DefinitionIdentity, Vec<DefinitionIdentity>>;

/// Ephemeral assessment dependencies, not a descriptor cache or a second inheritance model.
struct ReadinessPropagation {
    dependencies: Dependencies,
    reverse: Dependencies,
    names: HashMap<String, DefinitionIdentity>,
    candidate_names: HashSet<String>,
    labels: HashMap<DefinitionIdentity, String>,
    invalid: HashSet<DefinitionIdentity>,
    diagnosed: HashSet<String>,
    observed: usize,
}
impl ReadinessPropagation {
    fn invalidate(&mut self, identity: DefinitionIdentity) {
        let mut queue = VecDeque::from([identity]);
        while let Some(identity) = queue.pop_front() {
            if self.invalid.insert(identity.clone()) {
                queue.extend(self.reverse.get(&identity).into_iter().flatten().cloned());
            }
        }
    }
    fn observe(&mut self, collector: &ValidationCollector) {
        for finding in &collector.violations()[self.observed..] {
            if let Some((name, identity)) = crate::outcomes::subject_identity(&finding.subject)
                .and_then(|name| self.names.get(name).cloned().map(|identity| (name, identity)))
            {
                self.diagnosed.insert(name.to_string());
                // A candidate's own failure cannot invalidate its immutable saved definition.
                if !self.candidate_names.contains(name)
                    || matches!(identity, DefinitionIdentity::Staged(_))
                {
                    self.invalidate(identity);
                }
            }
        }
        self.observed = collector.violations().len();
    }
    fn blocking_dependency(&self, subject: &DefinitionIdentity) -> Option<&str> {
        self.dependencies
            .get(subject)
            .into_iter()
            .flatten()
            .find(|dependency| *dependency != subject && self.invalid.contains(*dependency))
            .and_then(|dependency| self.labels.get(dependency))
            .map(String::as_str)
    }
}

fn commitment_order(
    context: &Arc<TransactionContext>,
    subjects: &[SubjectPreparation],
    workset: &SchemaWorkset,
) -> Result<(Vec<Vec<usize>>, ReadinessPropagation), HolonError> {
    let mut dependencies = HashMap::new();
    // Each dependency node can have several independently assessed candidates.
    let mut definition_candidates = HashMap::<DefinitionIdentity, Vec<usize>>::new();
    let mut candidate_names = HashSet::new();
    let mut names = HashMap::new();
    let mut labels = HashMap::new();
    let mut pending = VecDeque::new();
    for (index, subject) in subjects.iter().enumerate() {
        definition_candidates
            .entry(DefinitionIdentity::for_reference(&subject.subject, context)?)
            .or_default()
            .push(index);
        candidate_names.insert(subject.subject.full_reference_id_string());
        pending.push_back(subject.subject.clone());
    }
    for view in &workset.schemas {
        pending.push_back(view.schema.clone());
    }
    while let Some(subject) = pending.pop_front() {
        let id = DefinitionIdentity::for_reference(&subject, context)?;
        names.insert(subject.full_reference_id_string(), id.clone());
        labels.entry(id.clone()).or_insert_with(|| subject.reference_id_string());
        if dependencies.contains_key(&id) {
            continue;
        }
        let mut edges = Vec::new();
        let definition = id.definition_reference(context, &subject);
        names.insert(definition.full_reference_id_string(), id.clone());
        // These are commitment dependencies. Arbitrary instance relationship
        // endpoints are not readiness dependencies; their policies belong to C4.
        for edge in [
            CoreRelationshipTypeName::DescribedBy,
            CoreRelationshipTypeName::Extends,
            CoreRelationshipTypeName::InstanceProperties,
            CoreRelationshipTypeName::InstanceRelationships,
            CoreRelationshipTypeName::ValueType,
            CoreRelationshipTypeName::Constraints,
            CoreRelationshipTypeName::ValidationBindings,
        ] {
            for target in targets(&definition, edge)? {
                edges.push(DefinitionIdentity::for_reference(&target, context)?);
                pending.push_back(target);
            }
        }
        for edge in [CoreRelationshipTypeName::ComponentOf, CoreRelationshipTypeName::RuleOf] {
            for owner in targets(&definition, edge)? {
                edges.push(DefinitionIdentity::for_reference(&owner, context)?);
                pending.push_back(owner);
            }
        }
        dependencies.insert(id, edges);
    }
    let mut reverse = HashMap::<_, Vec<_>>::new();
    for (subject, targets) in &dependencies {
        for target in targets {
            reverse.entry(target.clone()).or_default().push(subject.clone());
        }
    }
    let starts: Vec<_> = subjects
        .iter()
        .map(|subject| DefinitionIdentity::for_reference(&subject.subject, context))
        .collect::<Result<_, _>>()?;
    let order = crate::dependency_groups::dependency_groups(&starts, &dependencies)
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .flat_map(|id| definition_candidates.get(&id).cloned().unwrap_or_default())
                .collect()
        })
        .collect();
    Ok((
        order,
        ReadinessPropagation {
            dependencies,
            reverse,
            names,
            candidate_names,
            labels,
            invalid: HashSet::new(),
            diagnosed: HashSet::new(),
            observed: 0,
        },
    ))
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

#[cfg(test)]
mod readiness_tests {
    use super::*;
    use core_types::{CommitValidationViolation, ValidationSeverity, ValidationSubjectPath};

    /// The same collector path is used by conformance and future contextual capability checks.
    #[test]
    fn candidate_failure_only_invalidates_a_new_definition() {
        let saved = DefinitionIdentity::Saved(HolonId::Local(core_types::LocalId(vec![1; 39])));
        let successor = DefinitionIdentity::Staged(TemporaryId::default());
        let consumer = DefinitionIdentity::Staged(TemporaryId(
            "00000000-0000-0000-0000-000000000001".parse().unwrap(),
        ));
        for definition in [saved.clone(), successor.clone()] {
            let mut readiness = ReadinessPropagation {
                dependencies: HashMap::from([(consumer.clone(), vec![definition.clone()])]),
                reverse: HashMap::from([(definition.clone(), vec![consumer.clone()])]),
                names: HashMap::from([("candidate".into(), definition.clone())]),
                candidate_names: HashSet::from(["candidate".into()]),
                labels: HashMap::from([(definition.clone(), "definition".into())]),
                invalid: HashSet::new(),
                diagnosed: HashSet::new(),
                observed: 0,
            };
            let mut collector = ValidationCollector::default();
            collector.record(CommitValidationViolation {
                kind: CommitValidationViolationKind::RuleViolation {
                    code: "CandidateCheck".into(),
                },
                rule_key: None,
                severity: ValidationSeverity::Error,
                subject: ValidationSubjectPath::Holon { holon_identity: "candidate".into() },
                descriptor_identity: None,
                message: "candidate-specific failure".into(),
            });
            readiness.observe(&collector);
            assert_eq!(readiness.blocking_dependency(&consumer).is_some(), definition == successor);
            assert_eq!(readiness.invalid.contains(&definition), definition == successor);
            // A separately established immutable product failure does invalidate the definition.
            readiness.invalidate(definition.clone());
            assert!(readiness.blocking_dependency(&consumer).is_some());
        }
    }
}
