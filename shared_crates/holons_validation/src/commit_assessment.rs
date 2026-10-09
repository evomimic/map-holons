//! Commit assessment through prospective replacement selection.
use crate::{
    assessment_support::{blocked, path, recover, recover_transaction, targets},
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
use core_types::{CommitValidationViolationKind, HolonError};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    descriptors::{resolve_describing_type_with_reader, SchemaOwnershipKind},
    AssessmentReadError, ContractContributions, DescribingTypeResolution, DescriptorKindRoots,
    DescriptorReader, HolonReference, ProspectiveDescriptorReader, ProspectiveIdentity,
    StagedReference, StructuralPrerequisites, UniversalDescriptorContract,
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
        reader: &ProspectiveDescriptorReader,
    ) -> Result<Self, AssessmentReadError> {
        let resolve = |key| crate::resolve_validation_anchor_in_view(context, key, reader);
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
        let values = ValueValidationContext::resolve_in_view(context, reader)?;
        Ok(Self {
            kinds,
            contracts,
            constraints,
            schema: resolve("Schema.HolonType")?,
            rule: resolve("Rule.HolonType")?,
            values,
            universal: UniversalDescriptorContract::resolve_with_reader(context, reader)?,
        })
    }
}

struct SubjectPreparation {
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
    observer: impl FnMut(crate::AssessmentPhase),
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
    mut observer: impl FnMut(crate::AssessmentPhase),
) -> Result<(PreparedAssessment<'a>, ValidationObservations), HolonError> {
    observer(crate::AssessmentPhase::Construction);
    if candidates.is_empty() {
        return Ok((PreparedAssessment::default(), ValidationObservations::default()));
    }
    require_distinct_live_candidates(candidates)?;
    let reader = ProspectiveDescriptorReader::new(context, candidates)?;
    let mut collector = ValidationCollector::default();
    for finding in crate::competing_replacement_findings(&reader) {
        collector.record(finding);
    }
    let Some(roots) =
        recover_transaction(ReadinessContext::resolve(context, &reader), &mut collector)?
    else {
        for candidate in candidates {
            blocked(
                &mut collector,
                &candidate.into(),
                "Required validation anchors have contested prospective content.".into(),
            );
        }
        return finish_assessment(candidates, collector);
    };
    construct_assessment(context, candidates, &reader, &roots, collector)?.assess(&mut observer)
}

/// Preserve Commit's carrier grouping and finding order without installing outcomes.
fn finish_assessment<'a>(
    candidates: &'a [StagedReference],
    collector: ValidationCollector,
) -> Result<(PreparedAssessment<'a>, ValidationObservations), HolonError> {
    let (report, observations) = collector.into_parts();
    Ok((PreparedAssessment::from_scope(candidates, report)?, observations))
}

fn classify(governing: &holons_core::ValidExtendsLineage<'_>, root: &HolonReference) -> bool {
    governing.members().iter().any(|member| holons_core::same_definition(member, root))
}

/// Immutable prepared inputs and their cache leases live until assessment finishes.
pub(crate) struct ConstructedAssessment<'a, 'candidates> {
    context: &'a Arc<TransactionContext>,
    candidates: &'candidates [StagedReference],
    reader: &'a ProspectiveDescriptorReader,
    roots: &'a ReadinessContext,
    prepared: Vec<SubjectPreparation>,
    workset: SchemaWorkset,
    packages: crate::descriptor_package::DescriptorPackages,
    collector: ValidationCollector,
}

pub(crate) fn construct_assessment<'a, 'candidates>(
    context: &'a Arc<TransactionContext>,
    candidates: &'candidates [StagedReference],
    reader: &'a ProspectiveDescriptorReader,
    roots: &'a ReadinessContext,
    mut collector: ValidationCollector,
) -> Result<ConstructedAssessment<'a, 'candidates>, HolonError> {
    let kinds = roots.kinds.clone().with_reader(reader);
    let mut prepared = Vec::new();
    let mut owned = Vec::new();
    let mut schemas = Vec::new();
    // Structural diagnosis always precedes classification and binding discovery.
    for candidate in candidates {
        let subject = HolonReference::from(candidate);
        let Some(subject_lineage) = recover(
            holons_core::ExtendsLineageDiagnosis::assess_with_reader(
                &subject,
                &kinds.type_descriptor,
                reader,
            ),
            &subject,
            &mut collector,
        )?
        else {
            if let Some(saved) = crate::schema_view::saved_source(context, candidate)? {
                for kind in [SchemaOwnershipKind::Component, SchemaOwnershipKind::Rule] {
                    if !targets(&saved, crate::schema_view::ownership_edge(kind))?.is_empty() {
                        owned.push(OwnedCandidate { subject: subject.clone(), kind });
                    }
                }
            }
            continue;
        };
        let describing = recover(
            resolve_describing_type_with_reader(&subject, reader),
            &subject,
            &mut collector,
        )?;
        let describing_blocked = describing.is_none();
        let describing_type = describing.unwrap_or(DescribingTypeResolution::Missing);
        let governing_lineage =
            if let DescribingTypeResolution::Unique(descriptor) = &describing_type {
                if holons_core::same_definition(descriptor, &subject) {
                    Some(subject_lineage.clone())
                } else {
                    recover(
                        holons_core::ExtendsLineageDiagnosis::assess_with_reader(
                            descriptor,
                            &kinds.type_descriptor,
                            reader,
                        ),
                        &subject,
                        &mut collector,
                    )?
                }
            } else {
                None
            };
        let prerequisites =
            StructuralPrerequisites { describing_type, subject_lineage, governing_lineage };
        match &prerequisites.describing_type {
            DescribingTypeResolution::Missing if !describing_blocked => finding(
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
            DescribingTypeResolution::Unique(_) | DescribingTypeResolution::Missing => {}
        }
        let mut products = DescriptorRuleProducts::default();
        products.prepare_structure(&prerequisites, &kinds.type_descriptor);
        recover(products.prepare_kind(&prerequisites, &kinds, &reader), &subject, &mut collector)?;
        let is_descriptor = if let Some(lineage) = prerequisites.subject_lineage.valid_lineage() {
            recover(kinds.is_descriptor(lineage), &subject, &mut collector)?.unwrap_or(false)
        } else {
            false
        };
        let governing =
            prerequisites.governing_lineage.as_ref().and_then(|lineage| lineage.valid_lineage());
        let is_schema = governing.is_some_and(|lineage| classify(&lineage, &roots.schema));
        let is_constraint =
            governing.is_some_and(|lineage| classify(&lineage, &roots.constraints.constraint_type));
        let is_rule = governing.is_some_and(|lineage| classify(&lineage, &roots.rule));
        if is_schema {
            schemas.push(subject.clone());
        }
        for (kind, required) in [
            (SchemaOwnershipKind::Component, is_descriptor),
            (SchemaOwnershipKind::Rule, is_rule || is_constraint),
        ] {
            let edge = crate::schema_view::ownership_edge(kind);
            let old_membership =
                if let Some(saved) = crate::schema_view::saved_source(context, candidate)? {
                    !targets(&saved, edge.clone())?.is_empty()
                } else {
                    false
                };
            if required || old_membership || !targets(&subject, edge)?.is_empty() {
                owned.push(OwnedCandidate { subject: subject.clone(), kind });
            }
        }
        prepared.push(SubjectPreparation {
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
    let workset =
        SchemaWorkset::prepare(context, reader, &roots.schema, &schemas, &owned, &mut collector)?;
    let package_roots: Vec<_> = prepared
        .iter()
        .filter_map(|subject| match &subject.prerequisites.describing_type {
            DescribingTypeResolution::Unique(root) => Some(root.clone()),
            _ => None,
        })
        .collect();
    let package_subjects: Vec<_> = prepared.iter().map(|subject| subject.subject.clone()).collect();
    let packages = crate::descriptor_package::DescriptorPackages::construct(
        context,
        reader,
        &package_roots,
        &package_subjects,
        &workset,
    )?;
    Ok(ConstructedAssessment {
        context,
        candidates,
        reader,
        roots,
        prepared,
        workset,
        packages,
        collector,
    })
}

impl<'candidates> ConstructedAssessment<'_, 'candidates> {
    /// Consumes prepared packages and records findings without installing outcomes.
    pub(crate) fn assess(
        self,
        mut observer: impl FnMut(crate::AssessmentPhase),
    ) -> Result<(PreparedAssessment<'candidates>, ValidationObservations), HolonError> {
        observer(crate::AssessmentPhase::PackageValidation);
        let Self {
            context,
            candidates,
            reader,
            roots,
            mut prepared,
            workset,
            mut packages,
            mut collector,
        } = self;
        let kinds = roots.kinds.clone().with_reader(reader);
        let mut prepared_index = HashMap::new();
        for (index, subject) in prepared.iter().enumerate() {
            prepared_index
                .insert(ProspectiveIdentity::for_reference(&subject.subject, context)?, index);
        }
        // Defined contracts are prepared before any subject consumes them. Include unstaged
        // governors, whose malformed definitions must block rather than escape as a read error.
        let mut invalid = HashSet::new();
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
                let identity = ProspectiveIdentity::for_reference(&descriptor, context)?;
                if !checked_contracts.insert(identity.clone()) {
                    continue;
                }
                let mut products = DescriptorRuleProducts::default();
                let diagnosis = holons_core::ExtendsLineageDiagnosis::assess_with_reader(
                    &descriptor,
                    &kinds.type_descriptor,
                    reader,
                );
                let Some(diagnosis) = recover(diagnosis, &descriptor, &mut collector)? else {
                    invalid.insert(identity);
                    continue;
                };
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
                if let Some(contributions) = recover(
                    ContractContributions::resolve_with_reader(&descriptor, reader),
                    &descriptor,
                    &mut collector,
                )? {
                    recover(
                        products.prepare_contract(
                            &contributions,
                            &kinds,
                            &roots.contracts,
                            &reader,
                        ),
                        &descriptor,
                        &mut collector,
                    )?;
                }
                if products.has_findings() {
                    invalid.insert(identity.clone());
                }
                if let Some(index) = prepared_index.get(&identity) {
                    prepared[*index].products.append(products);
                } else if products.has_findings() {
                    blocked(&mut collector, &descriptor, "Saved governing contract has invalid effective member definitions; correct its descriptor commitments.".into());
                }
            }
        }
        // Each Schema gets its own declaration memo and collector scope. Shared reusable rules
        // are checked once in that Schema, with their original ownership and subject intact.
        let mut declaration_members = HashSet::new();
        for view in &workset.schemas {
            let Some(mut declarations) = recover(
                ConstraintDeclarationAssessment::new(&roots.constraints, reader),
                &view.schema,
                &mut collector,
            )?
            else {
                continue;
            };
            for descriptor in &view.components {
                declaration_members
                    .insert(ProspectiveIdentity::for_reference(descriptor, context)?);
                let Some(diagnosis) = recover(
                    holons_core::ExtendsLineageDiagnosis::assess_with_reader(
                        descriptor,
                        &kinds.type_descriptor,
                        reader,
                    ),
                    descriptor,
                    &mut collector,
                )?
                else {
                    continue;
                };
                let Some(lineage) = diagnosis.valid_lineage() else {
                    continue;
                };
                let mut products = DescriptorRuleProducts::default();
                let result = declarations.assess_descriptor(lineage, &mut products, &mut collector);
                recover(result, descriptor, &mut collector)?;
                if let Some(index) =
                    prepared_index.get(&ProspectiveIdentity::for_reference(descriptor, context)?)
                {
                    prepared[*index].products.append(products);
                }
            }
            for rule in &view.rules {
                declaration_members.insert(ProspectiveIdentity::for_reference(rule, context)?);
                assess_configured_rule(
                    rule,
                    &roots.constraints.constraint_type,
                    reader,
                    &mut declarations,
                    &mut collector,
                )?;
            }
        }
        // Invalid/missing ownership cannot remove a staged commitment from declaration assessment.
        let mut orphan_declarations = recover_transaction(
            ConstraintDeclarationAssessment::new(&roots.constraints, reader),
            &mut collector,
        )?;
        for subject in &mut prepared {
            if !declaration_members
                .contains(&ProspectiveIdentity::for_reference(&subject.subject, context)?)
            {
                if subject.is_descriptor {
                    if let Some(lineage) = subject.prerequisites.subject_lineage.valid_lineage() {
                        if let Some(declarations) = &mut orphan_declarations {
                            let result = declarations.assess_descriptor(
                                lineage,
                                &mut subject.products,
                                &mut collector,
                            );
                            recover(result, &subject.subject, &mut collector)?;
                        }
                    }
                } else if subject.is_constraint {
                    if let Some(declarations) = &mut orphan_declarations {
                        let result =
                            declarations.assess_constraint(&subject.subject, &mut collector);
                        recover(result, &subject.subject, &mut collector)?;
                    }
                }
            }
            if subject.products.has_findings() {
                invalid.insert(ProspectiveIdentity::for_reference(&subject.subject, context)?);
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
                    reader,
                    &path(&subject.subject),
                    &mut collector,
                    &packages,
                );
                subject.bindings =
                    recover(result, &subject.subject, &mut collector)?.unwrap_or_default();
            }
            subject_assessment::dispatch_descriptor(
                &subject.bindings,
                &subject.products,
                &subject.subject,
                &mut collector,
            )?;
        }
        let cycles =
            schema_rules::dependency_cycles(context, reader, &workset.schemas, &mut collector)?;
        for view in &workset.schemas {
            collector.observations.schema_assessment_count += 1;
            let mut products = SchemaRuleProducts::default();
            let id = ProspectiveIdentity::for_reference(&view.schema, context)?;
            if let Some(Some(witness)) = cycles.get(&id) {
                products.record(CoreValidationRuleName::SchemaDependenciesAcyclic, format!("Schema {} reaches a versioned DependsOn cycle containing {}; remove the cyclic dependency.", view.schema.reference_id_string(), witness.reference_id_string()));
            }
            schema_rules::cross_schema_references_prepared(
                context,
                reader,
                view,
                &workset,
                &mut products,
                &mut collector,
                &packages,
            )?;
            if products.has_findings() {
                invalid.insert(id.clone());
            }
            if let Some(index) = prepared_index.get(&id) {
                subject_assessment::dispatch_schema(
                    &prepared[*index].bindings,
                    &products,
                    &view.schema,
                    &mut collector,
                )?;
            } else if let Some(DescribingTypeResolution::Unique(descriptor)) = recover(
                resolve_describing_type_with_reader(&view.schema, reader),
                &view.schema,
                &mut collector,
            )? {
                let result = subject_assessment::prepare_bindings(
                    &descriptor,
                    SubjectLevel::Holon,
                    &roots.values,
                    reader,
                    &path(&view.schema),
                    &mut collector,
                    &packages,
                );
                if let Some(bindings) = recover(result, &view.schema, &mut collector)? {
                    subject_assessment::dispatch_schema(
                        &bindings,
                        &products,
                        &view.schema,
                        &mut collector,
                    )?;
                }
            }
        }
        // Preparation is complete before any subject consumes commitments. Readiness
        // propagation indexes references once and consumes only newly added findings.
        let (order, mut readiness) =
            commitment_order(context, reader, &prepared, &workset, &mut collector)?;
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
                    reader,
                    roots,
                    &packages,
                    &mut readiness,
                    &mut collector,
                )?;
            }
        }
        packages.finish(&readiness.invalid);
        observer(crate::AssessmentPhase::InstanceValidation);
        for group in &order {
            for &index in group {
                let subject = &prepared[index];
                if is_definition(subject) {
                    continue;
                }
                assess_candidate(
                    context,
                    subject,
                    reader,
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
            let id = ProspectiveIdentity::for_reference(&subject.subject, context)?;
            if readiness.invalid.contains(&id) && !readiness.diagnosed.contains(&id) {
                blocked(
                    &mut collector,
                    &subject.subject,
                    "A required prospective commitment did not establish readiness.".into(),
                );
            }
        }
        finish_assessment(candidates, collector)
    }
}

/// Definition groups use constructed peer contracts provisionally. Ordinary instances
/// reach this operation only after package readiness has been established.
fn assess_candidate(
    context: &Arc<TransactionContext>,
    subject: &SubjectPreparation,
    reader: &ProspectiveDescriptorReader,
    roots: &ReadinessContext,
    packages: &crate::descriptor_package::DescriptorPackages,
    readiness: &mut ReadinessPropagation,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let id = ProspectiveIdentity::for_reference(&subject.subject, context)?;
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
        } else if let Some(contract) =
            recover(package.contract.clone(), &subject.subject, collector)?
        {
            let result = subject_assessment::assess_holon(
                &subject.subject,
                &package.root,
                &contract,
                &subject.bindings,
                &roots.values,
                &roots.universal,
                reader,
                collector,
                packages,
            );
            recover(result, &subject.subject, collector)?;
        }
    }
    readiness.observe(collector);
    Ok(())
}

fn assess_configured_rule(
    rule: &HolonReference,
    constraint_root: &HolonReference,
    reader: &ProspectiveDescriptorReader,
    declarations: &mut ConstraintDeclarationAssessment<'_, ProspectiveDescriptorReader>,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let result = (|| {
        if let DescribingTypeResolution::Unique(descriptor) =
            resolve_describing_type_with_reader(rule, reader)?
        {
            if holons_core::descriptors::equals_or_extends_with_reader(
                &descriptor,
                constraint_root,
                reader,
            )? {
                declarations.assess_constraint(rule, collector)?;
            }
        }
        Ok::<_, AssessmentReadError>(())
    })();
    recover(result, rule, collector)?;
    Ok(())
}

type Dependencies = HashMap<ProspectiveIdentity, Vec<ProspectiveIdentity>>;

/// Ephemeral assessment dependencies, not a descriptor cache or a second inheritance model.
struct ReadinessPropagation {
    dependencies: Dependencies,
    reverse: Dependencies,
    names: HashMap<String, ProspectiveIdentity>,
    labels: HashMap<ProspectiveIdentity, String>,
    invalid: HashSet<ProspectiveIdentity>,
    diagnosed: HashSet<ProspectiveIdentity>,
    observed: usize,
}
impl ReadinessPropagation {
    fn invalidate(&mut self, identity: ProspectiveIdentity) {
        let mut queue = VecDeque::from([identity]);
        while let Some(identity) = queue.pop_front() {
            if self.invalid.insert(identity.clone()) {
                queue.extend(self.reverse.get(&identity).into_iter().flatten().cloned());
            }
        }
    }
    fn observe(&mut self, collector: &ValidationCollector) {
        for finding in &collector.violations()[self.observed..] {
            if let Some(identity) = crate::outcomes::subject_identity(&finding.subject)
                .and_then(|name| self.names.get(name))
                .cloned()
            {
                self.diagnosed.insert(identity.clone());
                self.invalidate(identity);
            }
        }
        self.observed = collector.violations().len();
    }
    fn blocking_dependency(&self, subject: &ProspectiveIdentity) -> Option<&str> {
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
    reader: &ProspectiveDescriptorReader,
    subjects: &[SubjectPreparation],
    workset: &SchemaWorkset,
    collector: &mut ValidationCollector,
) -> Result<(Vec<Vec<usize>>, ReadinessPropagation), HolonError> {
    let mut dependencies = HashMap::new();
    let mut indices = HashMap::new();
    let mut names = HashMap::new();
    let mut labels = HashMap::new();
    let mut pending = VecDeque::new();
    for (index, subject) in subjects.iter().enumerate() {
        indices.insert(ProspectiveIdentity::for_reference(&subject.subject, context)?, index);
        pending.push_back(subject.subject.clone());
    }
    for view in &workset.schemas {
        pending.push_back(view.schema.clone());
    }
    while let Some(subject) = pending.pop_front() {
        let id = ProspectiveIdentity::for_reference(&subject, context)?;
        names.insert(subject.reference_id_string(), id.clone());
        labels.entry(id.clone()).or_insert_with(|| subject.reference_id_string());
        if dependencies.contains_key(&id) {
            continue;
        }
        let mut edges = Vec::new();
        if let Some(selected) = recover(reader.select(&subject), &subject, collector)? {
            names.insert(selected.reference_id_string(), id.clone());
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
                for target in targets(&selected, edge)? {
                    edges.push(ProspectiveIdentity::for_reference(&target, context)?);
                    pending.push_back(target);
                }
            }
            for edge in [CoreRelationshipTypeName::ComponentOf, CoreRelationshipTypeName::RuleOf] {
                for owner in targets(&selected, edge)? {
                    edges.push(ProspectiveIdentity::for_reference(&owner, context)?);
                    pending.push_back(owner);
                }
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
        .map(|subject| ProspectiveIdentity::for_reference(&subject.subject, context))
        .collect::<Result<_, _>>()?;
    let order = crate::dependency_groups::dependency_groups(&starts, &dependencies)
        .into_iter()
        .map(|group| group.into_iter().filter_map(|id| indices.get(&id).copied()).collect())
        .collect();
    Ok((
        order,
        ReadinessPropagation {
            dependencies,
            reverse,
            names,
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
