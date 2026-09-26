//! Prepared descriptor facts and their fixed rule handlers.
//!
//! Preparation is split by family so a structural finding survives an unavailable
//! kind or contract product. Commit orchestration supplies one product set per
//! subject and selects prospective content before calling these methods.

use std::collections::BTreeMap;

use base_types::BaseValue;
use core_types::{CommitValidationViolationKind, HolonError};
use holons_core::{
    descriptors::{
        effective_relationship_targets_with_reader, same_definition, ContractContributions,
        DescriptorKindRoots, ExtendsLineageDefect, KindResolutionError, StructuralPrerequisites,
    },
    Descriptor, DescriptorReader, HolonReference, ReadableHolon,
};
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName, CoreValidationRuleName};

use crate::commitments::required_key;
use crate::handlers::{finding, rule_violation};
use crate::{ResolvedValidationBinding, RuleOutcome, ValidationCollector, ValidationInvocation};

#[derive(Clone, Copy, Debug)]
enum DiagnosticKind {
    Violation,
    UnresolvedDependency,
}

#[derive(Clone, Debug)]
struct PreparedDiagnostic {
    kind: DiagnosticKind,
    message: String,
}

/// Family roots required by DS-CONTRACT-004. The roots are resolved once by the
/// assessment and selected through the same reader as the member definitions.
#[derive(Clone, Debug)]
pub struct ContractKindRoots {
    /// Designated `PropertyType.TypeDescriptor` root.
    pub property: HolonReference,
    /// Designated `DeclaredRelationshipType.RelationshipType` root.
    pub relationship: HolonReference,
    /// Designated `ValueType.TypeDescriptor` root.
    pub value: HolonReference,
}

/// Findings prepared from kernel products, keyed by their canonical rule identity.
/// Each bound handler translates only its own entries into the common finding model.
/// Prepare each family once per subject, using the same reader and unchanged input
/// snapshot that supplied its kernel products. Discard the products on a read error.
#[derive(Default)]
pub struct DescriptorRuleProducts {
    diagnostics: BTreeMap<CoreValidationRuleName, Vec<PreparedDiagnostic>>,
}

impl DescriptorRuleProducts {
    pub(crate) fn violation(&mut self, rule: CoreValidationRuleName, message: String) {
        self.diagnostics
            .entry(rule)
            .or_default()
            .push(PreparedDiagnostic { kind: DiagnosticKind::Violation, message });
    }

    pub(crate) fn unresolved(&mut self, rule: CoreValidationRuleName, message: String) {
        self.diagnostics
            .entry(rule)
            .or_default()
            .push(PreparedDiagnostic { kind: DiagnosticKind::UnresolvedDependency, message });
    }

    /// Translate every drained subject-lineage defect, independently of classification.
    /// A rootless non-root holon is not a descriptor by kernel classification;
    /// incompatible meta-type selection is handled separately by DS-KIND-005.
    /// Governing-descriptor diagnosis is assessed separately by the prerequisite pass.
    pub fn prepare_structure(
        &mut self,
        prerequisites: &StructuralPrerequisites,
        root: &HolonReference,
    ) {
        use CoreValidationRuleName::*;
        let lineage = &prerequisites.subject_lineage;
        for defect in &lineage.defects {
            match defect {
                ExtendsLineageDefect::MultipleParents { subject, count } => self.violation(
                    AtMostOneDirectParent,
                    format!(
                        "Holon {} has {count} direct Extends parents; retain at most one.",
                        subject.reference_id_string()
                    ),
                ),
                ExtendsLineageDefect::Cycle { path, repeated_descriptor } => self.violation(
                    AcyclicExtendsLineage,
                    format!(
                        "Extends lineage repeats {repeated_descriptor} after [{}]; remove the cyclic parent edge.",
                        path.iter().map(|member| member.reference_id_string()).collect::<Vec<_>>().join(" -> ")
                    ),
                ),
                ExtendsLineageDefect::WrongTermination { terminal } => self.violation(
                    ExtendsLineageTerminatesAtTypeDescriptor,
                    format!(
                        "Parented Extends lineage terminates at {} instead of TypeDescriptor {}; connect it to the designated root.",
                        terminal.reference_id_string(),
                        root.reference_id_string()
                    ),
                ),
                ExtendsLineageDefect::RootHasParent => self.violation(
                    UniqueTypeDescriptorRoot,
                    format!(
                        "Designated TypeDescriptor root {} has an Extends parent; remove that edge.",
                        root.reference_id_string()
                    ),
                ),
            }
        }
    }

    /// Read only graph-derived kind products after structural diagnosis.
    /// Descriptor-only checks use the kernel's classification; universal
    /// compatibility checks still apply to ordinary holons.
    pub fn prepare_kind<R: DescriptorReader>(
        &mut self,
        prerequisites: &StructuralPrerequisites,
        roots: &DescriptorKindRoots<R>,
        reader: &R,
    ) -> Result<(), R::Error> {
        use CoreValidationRuleName::*;
        let Some(subject_lineage) = prerequisites.subject_lineage.valid_lineage() else {
            return Ok(());
        };
        let subject = reader.select(subject_lineage.subject())?;
        let is_descriptor = roots.is_descriptor(subject_lineage)?;
        if is_descriptor {
            let local_anchor = match subject
                .property_value(CorePropertyTypeName::DefinesInstanceTypeKind)?
            {
                Some(BaseValue::BooleanValue(value)) => Some(value.0),
                None => {
                    self.violation(LocalInstanceKindAnchorDesignation, format!(
                        "Descriptor {} lacks its local Boolean DefinesInstanceTypeKind designation.",
                        subject.reference_id_string()
                    ));
                    None
                }
                Some(value) => {
                    self.violation(LocalInstanceKindAnchorDesignation, format!(
                        "Descriptor {} has local DefinesInstanceTypeKind with value kind {:?}; a Boolean is required.",
                        subject.reference_id_string(),
                        value.kind(),
                    ));
                    None
                }
            };
            if local_anchor == Some(true) {
                match subject.property_value(CorePropertyTypeName::IsAbstractType)? {
                    Some(BaseValue::BooleanValue(value)) if value.0 => {}
                    other => {
                        let actual = match other {
                            Some(BaseValue::BooleanValue(value)) => format!("Boolean {}", value.0),
                            Some(value) => format!("{:?}", value.kind()),
                            None => "no value".into(),
                        };
                        self.violation(
                            InstanceKindAnchorsAreAbstract,
                            format!(
                                "Instance-kind anchor {} must have IsAbstractType true; found {actual}.",
                                subject.reference_id_string()
                            ),
                        );
                    }
                }
            }
            // A missing or malformed local designation is already diagnosed and
            // cannot be treated as a reliable nearest-kind result.
            if local_anchor.is_some() {
                let kind = match roots.assess_instance_type_kind(subject_lineage) {
                    Ok(kind) => Some(kind),
                    Err(KindResolutionError::InvalidDesignation { descriptor, .. }) => {
                        self.unresolved(TypeDescriptorRootKindException, format!(
                            "Descriptor {} depends on descriptor {} with an invalid local instance-kind designation.",
                            subject.reference_id_string(), descriptor.reference_id_string()
                        ));
                        None
                    }
                    Err(KindResolutionError::Read(error)) => return Err(error),
                };
                if let Some(kind) = kind {
                    let is_root =
                        same_definition(&subject, &reader.select(&roots.type_descriptor)?);
                    if (is_root && kind.is_some()) || (!is_root && kind.is_none()) {
                        self.violation(TypeDescriptorRootKindException, format!(
                            "Descriptor {} has instance kind {:?}; only TypeDescriptor {} may have none.",
                            subject.reference_id_string(),
                            kind.as_ref().map(|kind| kind.reference_id_string()),
                            roots.type_descriptor.reference_id_string()
                        ));
                    }
                }
            }
        }
        if let Some((_, governing_lineage)) = prerequisites.valid_lineages() {
            if is_descriptor != roots.is_meta_type(governing_lineage)? {
                self.violation(DescriptorMetaTypeCorrespondence, format!(
                    "Holon {} has incompatible descriptor/meta-type correspondence with its direct describer.",
                    subject.reference_id_string()
                ));
            }
        }
        let compatibility = match roots.assess_describing_lineages_compatible(prerequisites) {
            Ok(compatibility) => compatibility,
            Err(KindResolutionError::InvalidDesignation { descriptor, .. }) => {
                self.unresolved(
                    DescribingCategoryCompatibility,
                    format!(
                        "Holon {} depends on descriptor {} with an invalid instance-kind designation.",
                        subject.reference_id_string(), descriptor.reference_id_string()
                    ),
                );
                return Ok(());
            }
            Err(KindResolutionError::Read(error)) => return Err(error),
        };
        if let Some(compatibility) = compatibility {
            match compatibility.category_matches {
                Some(true) => {}
                Some(false) => self.violation(DescribingCategoryCompatibility, format!(
                    "Holon {} uses a describing type outside its graph-derived required category.",
                    subject.reference_id_string()
                )),
                None => self.unresolved(DescribingCategoryCompatibility, format!(
                    "Holon {} cannot resolve the required describing category because its instance-kind anchor has no unique direct describer.",
                    subject.reference_id_string()
                )),
            }
        }
        Ok(())
    }

    /// Assess the contract defined by this descriptor, preserving additive
    /// contribution identity and provenance before name normalization.
    ///
    /// DS-CONTRACT-003 covers these structural fields: required singular `TypeName` on
    /// each member; required singular `ValueType` on property members; and
    /// required singular `SourceType` and `TargetType` on relationship members.
    /// Optional scalar fields are singular by the property map representation.
    /// Constraint declaration configuration is checked by the Schema-scoped
    /// constraint declaration assessment, not here.
    /// Default/key/value policies continue in C3; inverse, endpoint, collection,
    /// and governed occurrence policies continue in C4.
    pub fn prepare_contract<R: DescriptorReader>(
        &mut self,
        contributions: &ContractContributions,
        kind_roots: &DescriptorKindRoots<R>,
        contract_roots: &ContractKindRoots,
        reader: &R,
    ) -> Result<(), R::Error> {
        self.prepare_namespace(
            &contributions.properties,
            &MemberNamespace {
                name: "property",
                expected_kind: &contract_roots.property,
                required_edges: &[(
                    CoreRelationshipTypeName::ValueType,
                    Some(&contract_roots.value),
                )],
            },
            kind_roots,
            reader,
        )?;
        self.prepare_namespace(
            &contributions.relationships,
            &MemberNamespace {
                name: "relationship",
                expected_kind: &contract_roots.relationship,
                required_edges: &[
                    (CoreRelationshipTypeName::SourceType, None),
                    (CoreRelationshipTypeName::TargetType, None),
                ],
            },
            kind_roots,
            reader,
        )?;
        Ok(())
    }

    fn prepare_namespace<R: DescriptorReader>(
        &mut self,
        contributions: &[holons_core::EffectiveRelationshipMember],
        definition: &MemberNamespace<'_>,
        kind_roots: &DescriptorKindRoots<R>,
        reader: &R,
    ) -> Result<(), R::Error> {
        use CoreValidationRuleName::*;
        let namespace = definition.name;
        let mut prior_members: Vec<(HolonReference, HolonReference)> = Vec::new();
        let mut names: Vec<(String, HolonReference, HolonReference)> = Vec::new();
        for contribution in contributions {
            let member = reader.select(&contribution.member)?;
            let declared_on = reader.select(&contribution.declared_on)?;
            if let Some((_, first_owner)) =
                prior_members.iter().find(|(prior, _)| same_definition(prior, &member))
            {
                if !same_definition(first_owner, &declared_on) {
                    self.violation(NoInheritedMemberRedeclaration, format!(
                        "Inherited {namespace} member {} was declared on both {} and {}; remove the later contribution.",
                        member.reference_id_string(),
                        first_owner.reference_id_string(),
                        declared_on.reference_id_string()
                    ));
                }
                continue;
            }
            prior_members.push((member.clone(), declared_on.clone()));
            match member.property_value(CorePropertyTypeName::TypeName)? {
                Some(BaseValue::StringValue(name)) if !name.0.is_empty() => {
                    if let Some((_, prior, first_owner)) =
                        names.iter().find(|(prior_name, _, _)| prior_name == &name.0)
                    {
                        self.violation(UniqueSemanticMemberNames, format!(
                            "Distinct {namespace} members {} and {} claim semantic name {}; declared on {} and {}, respectively.",
                            prior.reference_id_string(), member.reference_id_string(), name.0,
                            first_owner.reference_id_string(), declared_on.reference_id_string()
                        ));
                    } else {
                        names.push((name.0, member.clone(), declared_on.clone()));
                    }
                }
                other => {
                    let actual = other
                        .as_ref()
                        .map(|value| format!("{:?}", value.kind()))
                        .unwrap_or_else(|| "no value".into());
                    self.violation(WellFormedEffectiveMemberDefinitions, format!(
                        "Effective {namespace} member {} declared on {} requires one non-empty string TypeName; found {actual}.",
                        member.reference_id_string(), declared_on.reference_id_string()
                    ));
                }
            }
            for (edge, target_kind) in definition.required_edges {
                let targets = self.require_one_target(&member, edge.clone(), namespace, reader)?;
                if let (Some(expected), [target]) = (target_kind, targets.as_slice()) {
                    self.check_member_kind(
                        target,
                        expected,
                        &format!("{namespace} {} target", edge.as_relationship_name()),
                        kind_roots,
                        reader,
                    )?;
                }
            }
            self.check_member_kind(
                &member,
                definition.expected_kind,
                namespace,
                kind_roots,
                reader,
            )?;
        }
        Ok(())
    }

    fn require_one_target<R: DescriptorReader>(
        &mut self,
        member: &HolonReference,
        relationship: CoreRelationshipTypeName,
        namespace: &str,
        reader: &R,
    ) -> Result<Vec<HolonReference>, R::Error> {
        let targets =
            effective_relationship_targets_with_reader(member, relationship.clone(), reader)?
                .into_iter()
                .map(|contribution| contribution.member)
                .collect::<Vec<_>>();
        if targets.len() != 1 {
            self.violation(
                CoreValidationRuleName::WellFormedEffectiveMemberDefinitions,
                format!(
                    "Effective {namespace} member {} requires exactly one {} target; found {}.",
                    member.reference_id_string(),
                    relationship.as_relationship_name(),
                    targets.len()
                ),
            );
        }
        Ok(targets)
    }

    fn check_member_kind<R: DescriptorReader>(
        &mut self,
        member: &HolonReference,
        expected: &HolonReference,
        role: &str,
        roots: &DescriptorKindRoots<R>,
        reader: &R,
    ) -> Result<(), R::Error> {
        use holons_core::ExtendsLineageDiagnosis;
        let diagnosis =
            ExtendsLineageDiagnosis::assess_with_reader(member, &roots.type_descriptor, reader)?;
        let Some(lineage) = diagnosis.valid_lineage() else {
            self.unresolved(
                CoreValidationRuleName::ContractMemberKindCompatibility,
                format!(
                    "Cannot classify {role} {} until its Extends lineage is corrected.",
                    member.reference_id_string()
                ),
            );
            return Ok(());
        };
        let kind = match roots.assess_instance_type_kind(lineage) {
            Ok(kind) => kind,
            Err(KindResolutionError::InvalidDesignation { descriptor, .. }) => {
                self.unresolved(
                    CoreValidationRuleName::ContractMemberKindCompatibility,
                    format!(
                        "Cannot classify {role} {} until descriptor {} has a valid instance-kind designation.",
                        member.reference_id_string(), descriptor.reference_id_string()
                    ),
                );
                return Ok(());
            }
            Err(KindResolutionError::Read(error)) => return Err(error),
        };
        let expected = reader.select(expected)?;
        let compatible = match kind {
            Some(kind) => {
                // The resolved anchor comes from this already drained lineage.
                // Its suffix is exactly the anchor's ancestry; re-reading it would
                // repeat the same graph walk for every member sharing that anchor.
                lineage
                    .members()
                    .iter()
                    .skip_while(|ancestor| !same_definition(ancestor, &kind))
                    .any(|ancestor| same_definition(ancestor, &expected))
            }
            None => false,
        };
        if !compatible {
            self.violation(
                CoreValidationRuleName::ContractMemberKindCompatibility,
                format!(
                    "{role} {} must have an InstanceTypeKind equal to or specializing {}.",
                    member.reference_id_string(),
                    expected.reference_id_string()
                ),
            );
        }
        Ok(())
    }

    pub(crate) fn append(&mut self, other: Self) {
        for (rule, diagnostics) in other.diagnostics {
            self.diagnostics.entry(rule).or_default().extend(diagnostics);
        }
    }

    pub(crate) fn has_findings(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Universal prerequisites must remain reachable even when the chosen describer
    /// cannot inherit their bindings. Emit only rules not already dispatched here.
    pub(crate) fn emit_prerequisites(
        &self,
        path: &core_types::ValidationSubjectPath,
        dispatched: &std::collections::HashSet<CoreValidationRuleName>,
        collector: &mut ValidationCollector,
    ) {
        use CoreValidationRuleName::*;
        for (rule, code) in [
            (AtMostOneDirectParent, "DS-STRUCT-002"),
            (AcyclicExtendsLineage, "DS-STRUCT-003"),
            (ExtendsLineageTerminatesAtTypeDescriptor, "DS-STRUCT-004"),
            (UniqueTypeDescriptorRoot, "DS-STRUCT-005"),
            (DescribingCategoryCompatibility, "DS-KIND-004"),
            (DescriptorMetaTypeCorrespondence, "DS-KIND-005"),
        ] {
            if dispatched.contains(&rule) {
                continue;
            }
            for diagnostic in self.diagnostics.get(&rule).into_iter().flatten() {
                let kind = match diagnostic.kind {
                    DiagnosticKind::Violation => {
                        CommitValidationViolationKind::RuleViolation { code: code.into() }
                    }
                    DiagnosticKind::UnresolvedDependency => {
                        CommitValidationViolationKind::UnresolvedLocalDependency
                    }
                };
                finding(
                    collector,
                    kind,
                    Some(rule.as_str().into()),
                    path,
                    None,
                    diagnostic.message.clone(),
                );
            }
        }
    }

    fn emit(
        &self,
        rule: CoreValidationRuleName,
        code: &str,
        binding: &ResolvedValidationBinding,
        path: &core_types::ValidationSubjectPath,
        collector: &mut ValidationCollector,
    ) -> Result<RuleOutcome, HolonError> {
        for diagnostic in self.diagnostics.get(&rule).into_iter().flatten() {
            match diagnostic.kind {
                DiagnosticKind::Violation => {
                    rule_violation(collector, binding, path, code, diagnostic.message.clone())?;
                }
                DiagnosticKind::UnresolvedDependency => {
                    finding(
                        collector,
                        CommitValidationViolationKind::UnresolvedLocalDependency,
                        Some(required_key(&binding.rule)?),
                        path,
                        Some(binding.declaring_descriptor.holon().reference_id_string()),
                        diagnostic.message.clone(),
                    );
                }
            }
        }
        Ok(RuleOutcome::Continue)
    }
}

/// Required structural edges and any kind policy for their selected targets.
/// Endpoint compatibility is deliberately absent: it belongs to C4.
struct MemberNamespace<'a> {
    name: &'static str,
    expected_kind: &'a HolonReference,
    required_edges: &'a [(CoreRelationshipTypeName, Option<&'a HolonReference>)],
}

fn run(
    invocation: ValidationInvocation<'_>,
    collector: &mut ValidationCollector,
    rule: CoreValidationRuleName,
    code: &str,
) -> Result<RuleOutcome, HolonError> {
    let ValidationInvocation::Descriptor { binding, path, products } = invocation else {
        return Err(HolonError::InvalidParameter(
            "Descriptor rule requires prepared descriptor products".into(),
        ));
    };
    products.emit(rule, code, binding, path, collector)
}

pub(crate) fn at_most_one_direct_parent(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::AtMostOneDirectParent, "DS-STRUCT-002")
}
pub(crate) fn acyclic_extends_lineage(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::AcyclicExtendsLineage, "DS-STRUCT-003")
}
pub(crate) fn extends_lineage_terminates_at_type_descriptor(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::ExtendsLineageTerminatesAtTypeDescriptor, "DS-STRUCT-004")
}
pub(crate) fn unique_type_descriptor_root(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::UniqueTypeDescriptorRoot, "DS-STRUCT-005")
}
pub(crate) fn local_instance_kind_anchor_designation(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::LocalInstanceKindAnchorDesignation, "DS-KIND-001")
}
pub(crate) fn instance_kind_anchors_are_abstract(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::InstanceKindAnchorsAreAbstract, "DS-KIND-002")
}
pub(crate) fn type_descriptor_root_kind_exception(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::TypeDescriptorRootKindException, "DS-KIND-003")
}
pub(crate) fn describing_category_compatibility(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::DescribingCategoryCompatibility, "DS-KIND-004")
}
pub(crate) fn descriptor_meta_type_correspondence(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::DescriptorMetaTypeCorrespondence, "DS-KIND-005")
}
pub(crate) fn no_inherited_member_redeclaration(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::NoInheritedMemberRedeclaration, "DS-CONTRACT-001")
}
pub(crate) fn unique_semantic_member_names(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::UniqueSemanticMemberNames, "DS-CONTRACT-002")
}
pub(crate) fn well_formed_effective_member_definitions(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::WellFormedEffectiveMemberDefinitions, "DS-CONTRACT-003")
}
pub(crate) fn contract_member_kind_compatibility(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::ContractMemberKindCompatibility, "DS-CONTRACT-004")
}

/// Dispatches the prepared combined-set preservation result, without evaluating any constraint.
pub(crate) fn inherited_constraint_non_relaxation(
    invocation: ValidationInvocation<'_>,
    collector: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(
        invocation,
        collector,
        CoreValidationRuleName::InheritedValueConstraintNonRelaxation,
        "DS-CONSTRAINT-001",
    )
}
