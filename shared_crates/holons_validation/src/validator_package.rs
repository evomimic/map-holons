//! Bounded instance contract construction. Interpretation demand never implies validation
//! of a supporting definition as a subject, ownership expansion, or instance-target expansion.
use crate::{
    assessment_support::targets,
    descriptor_package::{
        content_identity, DescriptorPackage, DescriptorPackages, PackageState, PreparedDefinition,
    },
    schema_view::SchemaWorkset,
    AssessmentEvent, AssessmentPhase, PreparationMetrics,
};
use core_types::HolonError;
use holons_core::{
    core_shared_objects::transactions::TransactionContext, AssessmentReadError,
    ContractContributions, DescriptorReader, HolonReference, ProspectiveDescriptorReader,
    ProspectiveIdentity, ReadableHolon,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
};
use type_names::CoreRelationshipTypeName as R;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Demand {
    Holon,
    Property,
    Value,
    Relationship,
    Rule,
    Family,
    Constraint,
    ConstraintType,
    Defaults,
    DefaultMember,
}
impl Demand {
    fn label(self) -> &'static str {
        match self {
            Self::Holon => "ValidatorRoot",
            Self::Property => "PropertyDeclaration",
            Self::Value => "ValueDescriptor",
            Self::Relationship => "RelationshipDeclaration",
            Self::Rule => "RuleBinding",
            Self::Family => "RuleFamily",
            Self::Constraint => "ConfiguredConstraint",
            Self::ConstraintType => "ConstraintType",
            Self::Defaults => "RequirednessContract",
            Self::DefaultMember => "RequirednessMember",
        }
    }
    fn edges(self, primary: bool) -> Vec<R> {
        match self {
            Self::Holon => vec![
                R::Extends,
                R::InstanceProperties,
                R::InstanceRelationships,
                R::Constraints,
                R::ValidationBindings,
            ],
            Self::Property => {
                let mut edges =
                    vec![R::Extends, R::ValueType, R::Constraints, R::ValidationBindings];
                if primary {
                    edges.push(R::DescribedBy);
                }
                edges
            }
            Self::Value => vec![R::Extends, R::Constraints, R::ValidationBindings],
            Self::Relationship => vec![
                R::Extends,
                R::SourceType,
                R::TargetType,
                R::Constraints,
                R::ValidationBindings,
            ],
            Self::Rule | Self::Constraint => vec![R::DescribedBy],
            Self::Family | Self::ConstraintType | Self::DefaultMember => vec![R::Extends],
            Self::Defaults => vec![R::Extends, R::InstanceProperties],
        }
    }
    fn next(self, edge: &R) -> Option<(Self, bool)> {
        match edge {
            R::Extends => Some((self, false)),
            R::InstanceProperties if self == Self::Defaults => Some((Self::DefaultMember, false)),
            R::InstanceProperties => Some((Self::Property, true)),
            R::InstanceRelationships => Some((Self::Relationship, true)),
            R::ValueType => Some((Self::Value, true)),
            R::Constraints => Some((Self::Constraint, false)),
            R::ValidationBindings => Some((Self::Rule, false)),
            R::DescribedBy if self == Self::Property => Some((Self::Defaults, false)),
            R::DescribedBy if self == Self::Rule => Some((Self::Family, false)),
            R::DescribedBy if self == Self::Constraint => Some((Self::ConstraintType, false)),
            _ => None,
        }
    }
}

/// The package owns schema semantics and retained supporting definitions, not subject data.
/// The existing assessment lookup/state adapter is reused during this bounded migration.
pub(crate) struct ValidatorPackage {
    packages: HashMap<ProspectiveIdentity, DescriptorPackage>,
    definitions: HashMap<ProspectiveIdentity, PreparedDefinition>,
    content: Vec<holons_core::core_shared_objects::holon_cache::SavedHolonRetention>,
    dependencies: HashMap<ProspectiveIdentity, Vec<HolonReference>>,
    preparation: PreparationMetrics,
}
impl ValidatorPackage {
    pub(crate) fn supports(
        reader: &ProspectiveDescriptorReader,
        roots: &[HolonReference],
        subjects: &[HolonReference],
        workset: &SchemaWorkset,
    ) -> Result<bool, HolonError> {
        if roots.is_empty() || !workset.schemas.is_empty() {
            return Ok(false);
        }
        for root in roots {
            let Ok(selected) = reader.select(root) else {
                return Ok(false);
            };
            if selected.key()?.is_none_or(|key| key.0 != "VisualizerUsage.HolonType") {
                return Ok(false);
            }
        }
        for subject in subjects {
            if !matches!(subject, HolonReference::Staged(_))
                || !targets(subject, R::Extends)?.is_empty()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(crate) fn construct(
        context: &Arc<TransactionContext>,
        reader: &ProspectiveDescriptorReader,
        roots: &[HolonReference],
        subjects: &[HolonReference],
        observer: &mut impl FnMut(AssessmentEvent),
    ) -> Result<Self, HolonError> {
        observer(AssessmentEvent::Phase(AssessmentPhase::PackageDiscovery));
        observer(AssessmentEvent::PreparationOperation {
            operation: "ValidatorPackageSelection",
            role: "ValidatorPackage",
            starting: true,
        });
        observer(AssessmentEvent::PreparationOperation {
            operation: "ValidatorPackageSelection",
            role: "ValidatorPackage",
            starting: false,
        });
        let mut pending: VecDeque<_> =
            roots.iter().cloned().map(|root| (root, Demand::Holon, true)).collect();
        let mut visited = HashSet::new();
        let mut distinct = HashSet::new();
        let mut products = HashMap::new();
        let mut content = Vec::new();
        let mut dependencies = HashMap::<ProspectiveIdentity, Vec<HolonReference>>::new();
        let mut edge_counts = BTreeMap::<(&str, String), (usize, usize)>::new();
        while let Some((reference, demand, primary)) = pending.pop_front() {
            let selected = match reader.select(&reference) {
                Ok(selected) => selected,
                Err(AssessmentReadError::Contested { .. }) => continue,
                Err(AssessmentReadError::Operational(error)) => return Err(error),
                Err(error) => return Err(HolonError::CommitFailure(error.to_string())),
            };
            let identity = content_identity(&selected);
            if primary {
                products.insert(identity.clone(), (selected.clone(), demand));
            }
            if !visited.insert((identity.clone(), demand, primary)) {
                continue;
            }
            observer(AssessmentEvent::PreparationVisitStarted);
            let first = distinct.insert(identity);
            if first {
                if let HolonReference::Smart(saved) = &selected {
                    observer(AssessmentEvent::PreparationOperation {
                        operation: "ContentRetention",
                        role: "SavedContent",
                        starting: true,
                    });
                    let retained = saved.retain_content();
                    observer(AssessmentEvent::PreparationOperation {
                        operation: "ContentRetention",
                        role: "SavedContent",
                        starting: false,
                    });
                    content.push(retained?);
                }
            }
            let edges = demand.edges(primary);
            if let HolonReference::Smart(saved) = &selected {
                observer(AssessmentEvent::PreparationOperation {
                    operation: "MembershipPrefetch",
                    role: demand.label(),
                    starting: true,
                });
                let prepared = saved.prepare_relationships(
                    &edges.iter().map(R::as_relationship_name).collect::<Vec<_>>(),
                );
                observer(AssessmentEvent::PreparationOperation {
                    operation: "MembershipPrefetch",
                    role: demand.label(),
                    starting: false,
                });
                prepared?;
            }
            let source = ProspectiveIdentity::for_reference(&selected, context)?;
            for edge in edges {
                let members = targets(&selected, edge.clone())?;
                let count = edge_counts
                    .entry((demand.label(), edge.as_relationship_name().to_string()))
                    .or_default();
                count.0 += 1;
                count.1 += members.len();
                dependencies.entry(source.clone()).or_default().extend(members.iter().cloned());
                if let Some((next, primary)) = demand.next(&edge) {
                    pending.extend(members.into_iter().map(|member| (member, next, primary)));
                }
            }
            observer(AssessmentEvent::PreparationVisitFinished {
                type_name: demand.label().into(),
                distinct_content: first,
            });
        }
        // Subject preparation is separate: only its governing reference is a commitment dependency.
        for subject in subjects {
            dependencies.insert(
                ProspectiveIdentity::for_reference(subject, context)?,
                targets(subject, R::DescribedBy)?,
            );
        }
        let mut definitions = HashMap::new();
        observer(AssessmentEvent::Phase(AssessmentPhase::PreparedProducts));
        for (identity, (reference, demand)) in products {
            let contract = if demand == Demand::Holon {
                observer(AssessmentEvent::PreparationOperation {
                    operation: "EffectiveContract",
                    role: "PreparedDefinition",
                    starting: true,
                });
                let result =
                    ContractContributions::resolve_with_reader(&reference, reader).map(Arc::new);
                observer(AssessmentEvent::PreparationOperation {
                    operation: "EffectiveContract",
                    role: "PreparedDefinition",
                    starting: false,
                });
                result
            } else {
                Err(HolonError::InvalidState(
                    "A supporting declaration does not supply the instance's holon contract".into(),
                )
                .into())
            };
            let constraints = holons_core::descriptors::effective_relationship_targets_with_reader(
                &reference,
                R::Constraints,
                reader,
            );
            let bindings = holons_core::descriptors::effective_relationship_targets_with_reader(
                &reference,
                R::ValidationBindings,
                reader,
            );
            definitions.insert(
                identity,
                PreparedDefinition {
                    constraints_consumed: Cell::new(false),
                    bindings_consumed: Cell::new(false),
                    contract,
                    constraints,
                    bindings,
                },
            );
        }
        let mut packages = HashMap::new();
        for root in roots {
            let contract = reader.select(root).and_then(|selected| {
                definitions
                    .get(&content_identity(&selected))
                    .map(|definition| definition.contract.clone())
                    .unwrap_or_else(|| {
                        Err(HolonError::InvalidState("Validator root was not prepared".into())
                            .into())
                    })
            });
            packages.insert(
                ProspectiveIdentity::for_reference(root, context)?,
                DescriptorPackage {
                    root: root.clone(),
                    contract,
                    state: PackageState::Constructed,
                    consumed: Cell::new(false),
                },
            );
        }
        let preparation = PreparationMetrics {
            package_roots: packages.len(),
            subjects: subjects.len(),
            distinct_content: distinct.len(),
            identity_role_visits: visited.len(),
            ..Default::default()
        };
        for ((role, edge), (requests, members)) in edge_counts {
            observer(AssessmentEvent::PreparationEdge { role, edge, requests, members });
        }
        observer(AssessmentEvent::Phase(AssessmentPhase::SchemaOwnershipPreparation));
        Ok(Self { packages, definitions, content, dependencies, preparation })
    }

    pub(crate) fn into_assessment_inputs(self) -> DescriptorPackages {
        DescriptorPackages {
            packages: self.packages,
            definitions: self.definitions,
            authored: HashMap::new(),
            _content: self.content,
            preparation: self.preparation,
            authored_consumed: RefCell::default(),
            planned_dependencies: Some(self.dependencies),
        }
    }
}

#[cfg(test)]
#[path = "validator_package_tests.rs"]
mod tests;
