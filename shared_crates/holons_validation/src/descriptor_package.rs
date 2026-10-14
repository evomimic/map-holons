//! Assessment-local effective contracts. Holon content remains in bound references/caches.
use crate::assessment_support::targets;
use crate::schema_view::SchemaWorkset;
use core_types::{HolonError, RelationshipName};
use holons_core::{
    core_shared_objects::transactions::TransactionContext, AssessmentReadError,
    ContractContributions, DescriptorReader, EffectiveRelationshipMember, HolonReference,
    ProspectiveDescriptorReader, ProspectiveIdentity, ReadableHolon,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
};
use type_names::CoreRelationshipTypeName as R;

/// State is scoped to this assessment, never persisted as schema acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PackageState {
    Constructing,
    Constructed,
    Validated,
    Blocked,
}

/// The root's effective contract retains kernel contribution ordering and provenance.
pub(crate) struct DescriptorPackage {
    pub root: HolonReference,
    pub contract: Result<Arc<ContractContributions>, AssessmentReadError>,
    pub state: PackageState,
    pub(super) consumed: Cell<bool>,
}

/// Package roots share the same prepared saved memberships and prospective definitions.
/// Entries are discarded before another assessment or schema mutation.
pub(crate) struct DescriptorPackages {
    pub(super) packages: HashMap<ProspectiveIdentity, DescriptorPackage>,
    pub(super) definitions: HashMap<ProspectiveIdentity, PreparedDefinition>,
    pub(super) authored: HashMap<
        ProspectiveIdentity,
        Result<Vec<(RelationshipName, HolonReference)>, AssessmentReadError>,
    >,
    pub(super) _content: Vec<holons_core::core_shared_objects::holon_cache::SavedHolonRetention>,
    pub(super) preparation: crate::PreparationMetrics,
    pub(super) authored_consumed: RefCell<HashSet<ProspectiveIdentity>>,
    #[cfg(feature = "validator-package-experiment")]
    pub(super) planned_dependencies: Option<HashMap<ProspectiveIdentity, Vec<HolonReference>>>,
}

/// Shared kernel products; configuration and other domain values remain on the holons.
pub(super) struct PreparedDefinition {
    pub(super) constraints_consumed: Cell<bool>,
    pub(super) bindings_consumed: Cell<bool>,
    pub(super) contract: Result<Arc<ContractContributions>, AssessmentReadError>,
    pub(super) constraints: Result<Vec<EffectiveRelationshipMember>, AssessmentReadError>,
    pub(super) bindings: Result<Vec<EffectiveRelationshipMember>, AssessmentReadError>,
}

/// Content identity preserves explicit competing staged candidates without abbreviated labels.
pub(super) fn content_identity(reference: &HolonReference) -> ProspectiveIdentity {
    match reference {
        HolonReference::Smart(saved) => ProspectiveIdentity::Saved(saved.holon_id()),
        HolonReference::Staged(staged) => ProspectiveIdentity::Staged(staged.temporary_id()),
        HolonReference::Transient(transient) => {
            ProspectiveIdentity::Transient(transient.temporary_id())
        }
    }
}

/// Diagnostic type label without fetching content solely for profiling.
fn cached_type_name(reference: &HolonReference) -> Option<String> {
    match reference {
        HolonReference::Smart(saved) => saved.smart_property_values().and_then(|properties| {
            properties.iter().find_map(|(name, value)| {
                if name.to_string() == "Key" {
                    if let core_types::BaseValue::StringValue(key) = value {
                        return Some(key.to_string());
                    }
                }
                None
            })
        }),
        _ => reference.key().ok().flatten().map(|key| key.to_string()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum PreparationRole {
    Commitment,
    RelationshipMember,
    Constraint,
    ConstraintType,
    Schema,
}

impl DescriptorPackages {
    #[cfg(test)]
    pub fn construct(
        context: &Arc<TransactionContext>,
        reader: &ProspectiveDescriptorReader,
        roots: &[HolonReference],
        subjects: &[HolonReference],
        workset: &SchemaWorkset,
    ) -> Result<Self, HolonError> {
        Self::construct_with_observer(context, reader, roots, subjects, workset, &mut |_| {})
    }

    pub fn construct_with_observer(
        context: &Arc<TransactionContext>,
        reader: &ProspectiveDescriptorReader,
        roots: &[HolonReference],
        subjects: &[HolonReference],
        workset: &SchemaWorkset,
        observer: &mut impl FnMut(crate::AssessmentEvent),
    ) -> Result<Self, HolonError> {
        #[cfg(feature = "validator-package-experiment")]
        if crate::validator_package::ValidatorPackage::supports(reader, roots, subjects, workset)? {
            return crate::validator_package::ValidatorPackage::construct(
                context, reader, roots, subjects, observer,
            )
            .map(crate::validator_package::ValidatorPackage::into_assessment_inputs);
        }
        Self::construct_descriptor_with_observer(
            context, reader, roots, subjects, workset, observer,
        )
    }

    pub(crate) fn construct_descriptor_with_observer(
        context: &Arc<TransactionContext>,
        reader: &ProspectiveDescriptorReader,
        roots: &[HolonReference],
        subjects: &[HolonReference],
        workset: &SchemaWorkset,
        observer: &mut impl FnMut(crate::AssessmentEvent),
    ) -> Result<Self, HolonError> {
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::PackageDiscovery));
        let mut edge_demand: BTreeMap<(&str, String), (usize, usize)> = BTreeMap::new();
        let mut packages = HashMap::new();
        for root in roots {
            let identity = ProspectiveIdentity::for_reference(root, context)?;
            packages.entry(identity).or_insert_with(|| DescriptorPackage {
                root: root.clone(),
                contract: Err(HolonError::InvalidState(
                    "Descriptor package is still constructing".into(),
                )
                .into()),
                state: PackageState::Constructing,
                consumed: Cell::new(false),
            });
        }

        use PreparationRole::*;
        let mut pending: VecDeque<_> = roots
            .iter()
            .chain(subjects)
            .cloned()
            .map(|reference| (reference, Commitment))
            .collect();
        for schema in &workset.schemas {
            pending.push_back((schema.schema.clone(), Schema));
            pending
                .extend(schema.components.iter().cloned().map(|reference| (reference, Commitment)));
            // Rules may be configured constraints; their applicability must be prepared too.
            pending.extend(schema.rules.iter().cloned().map(|reference| (reference, Constraint)));
        }
        let mut seen = HashSet::new();
        let mut content_seen = HashSet::new();
        let mut definitions_to_prepare = Vec::new();
        let mut content = Vec::new();
        while let Some((reference, role)) = pending.pop_front() {
            let selected = match reader.select(&reference) {
                Ok(selected) => selected,
                Err(AssessmentReadError::Contested { .. }) => continue,
                Err(AssessmentReadError::Operational(error)) => return Err(error),
                Err(error) => return Err(HolonError::CommitFailure(error.to_string())),
            };
            let identity = content_identity(&selected);
            if !seen.insert((identity.clone(), role)) {
                continue;
            }
            observer(crate::AssessmentEvent::PreparationVisitStarted);
            let distinct_content = content_seen.insert(identity);
            let mut type_name = "MissingDescribedBy".to_string();
            if distinct_content {
                definitions_to_prepare.push(selected.clone());
                if let HolonReference::Smart(saved) = &selected {
                    observer(crate::AssessmentEvent::PreparationOperation {
                        operation: "ContentRetention",
                        role: "SavedContent",
                        starting: true,
                    });
                    let retained = saved.retain_content();
                    observer(crate::AssessmentEvent::PreparationOperation {
                        operation: "ContentRetention",
                        role: "SavedContent",
                        starting: false,
                    });
                    content.push(retained?);
                }
            }
            // These are the existing readiness commitments, not arbitrary instance edges.
            let mut edges = vec![
                R::DescribedBy,
                R::Extends,
                R::InstanceProperties,
                R::InstanceRelationships,
                R::ValueType,
                R::Constraints,
                R::ValidationBindings,
                R::ComponentOf,
                R::RuleOf,
            ];
            match role {
                RelationshipMember => edges.extend([R::SourceType, R::TargetType]),
                ConstraintType => edges.push(R::ApplicableToDescriptorTypes),
                Schema => edges.push(R::DependsOn),
                _ => {}
            }
            let role_name = match role {
                Commitment => "Commitment",
                RelationshipMember => "RelationshipMember",
                Constraint => "Constraint",
                ConstraintType => "ConstraintType",
                Schema => "Schema",
            };
            if let HolonReference::Smart(saved) = &selected {
                observer(crate::AssessmentEvent::PreparationOperation {
                    operation: "MembershipPrefetch",
                    role: role_name,
                    starting: true,
                });
                let preparation = saved.prepare_relationships(
                    &edges.iter().map(|edge| edge.as_relationship_name()).collect::<Vec<_>>(),
                );
                observer(crate::AssessmentEvent::PreparationOperation {
                    operation: "MembershipPrefetch",
                    role: role_name,
                    starting: false,
                });
                preparation?;
            }
            observer(crate::AssessmentEvent::PreparationOperation {
                operation: "RelationshipTraversal",
                role: role_name,
                starting: true,
            });
            for edge in edges {
                let members = targets(&selected, edge.clone())?;
                if matches!(edge, R::DescribedBy) {
                    type_name = match members.as_slice() {
                        [] => "MissingDescribedBy".to_string(),
                        [descriptor] => cached_type_name(descriptor)
                            .unwrap_or_else(|| "UnlabeledDescribedBy".to_string()),
                        _ => "MultipleDescribedBy".to_string(),
                    };
                }
                let demand = edge_demand
                    .entry((role_name, edge.as_relationship_name().to_string()))
                    .or_default();
                demand.0 += 1;
                demand.1 += members.len();
                // C2 checks these endpoint/applicability identities without requiring the
                // referenced holon's full contract. Ownership demand is added separately.
                if matches!(edge, R::SourceType | R::TargetType | R::ApplicableToDescriptorTypes) {
                    continue;
                }
                let next_role = match edge {
                    R::InstanceRelationships => RelationshipMember,
                    R::Constraints => Constraint,
                    R::DescribedBy if role == Constraint => ConstraintType,
                    R::ComponentOf | R::RuleOf | R::DependsOn => Schema,
                    // Inherited declaration fields still need the member's specialized demand.
                    R::Extends if matches!(role, RelationshipMember | ConstraintType) => role,
                    _ => Commitment,
                };
                pending.extend(members.into_iter().map(|reference| (reference, next_role)));
            }
            observer(crate::AssessmentEvent::PreparationOperation {
                operation: "RelationshipTraversal",
                role: role_name,
                starting: false,
            });
            observer(crate::AssessmentEvent::PreparationVisitFinished {
                type_name,
                distinct_content,
            });
        }
        let preparation = crate::PreparationMetrics {
            package_roots: packages.len(),
            subjects: subjects.len(),
            affected_schemas: workset.schemas.len(),
            distinct_content: definitions_to_prepare.len(),
            identity_role_visits: seen.len(),
            ..Default::default()
        };
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::PreparedProducts));
        let mut definitions = HashMap::new();
        for reference in definitions_to_prepare {
            observer(crate::AssessmentEvent::PreparationOperation {
                operation: "EffectiveContract",
                role: "PreparedDefinition",
                starting: true,
            });
            let contract =
                ContractContributions::resolve_with_reader(&reference, reader).map(Arc::new);
            observer(crate::AssessmentEvent::PreparationOperation {
                operation: "EffectiveContract",
                role: "PreparedDefinition",
                starting: false,
            });
            observer(crate::AssessmentEvent::PreparationOperation {
                operation: "EffectiveConstraints",
                role: "PreparedDefinition",
                starting: true,
            });
            let constraints = holons_core::descriptors::effective_relationship_targets_with_reader(
                &reference,
                R::Constraints,
                reader,
            );
            observer(crate::AssessmentEvent::PreparationOperation {
                operation: "EffectiveConstraints",
                role: "PreparedDefinition",
                starting: false,
            });
            observer(crate::AssessmentEvent::PreparationOperation {
                operation: "EffectiveBindings",
                role: "PreparedDefinition",
                starting: true,
            });
            let bindings = holons_core::descriptors::effective_relationship_targets_with_reader(
                &reference,
                R::ValidationBindings,
                reader,
            );
            observer(crate::AssessmentEvent::PreparationOperation {
                operation: "EffectiveBindings",
                role: "PreparedDefinition",
                starting: false,
            });
            definitions.insert(
                content_identity(&reference),
                PreparedDefinition {
                    constraints_consumed: Cell::new(false),
                    bindings_consumed: Cell::new(false),
                    contract,
                    constraints,
                    bindings,
                },
            );
        }
        for root in roots {
            let identity = ProspectiveIdentity::for_reference(root, context)?;
            let entry = packages.get_mut(&identity).expect("root registered before discovery");
            entry.contract = match reader.select(root) {
                Ok(selected) => definitions
                    .get(&content_identity(&selected))
                    .ok_or_else(|| HolonError::InvalidState("Missing prepared descriptor".into()))?
                    .contract
                    .clone(),
                Err(error) => Err(error),
            };
            entry.state = PackageState::Constructed;
        }
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::SchemaOwnershipPreparation));
        let mut authored = HashMap::new();
        // Cross-schema checks inspect the complete declared surface of affected definitions.
        // Its targets require ownership reads, not recursive instance validation.
        for schema in &workset.schemas {
            for source in schema.components.iter().chain(&schema.rules) {
                let edges = crate::schema_rules::authored_targets(source, reader);
                if let Ok(selected) = reader.select(source) {
                    authored.insert(content_identity(&selected), edges.clone());
                }
                match edges {
                    Ok(edges) => {
                        for (_, target) in edges {
                            match reader.select(&target) {
                                Ok(HolonReference::Smart(saved)) => {
                                    saved.prepare_relationships(&[
                                        R::ComponentOf.as_relationship_name(),
                                        R::RuleOf.as_relationship_name(),
                                    ])?
                                }
                                Err(AssessmentReadError::Operational(error)) => return Err(error),
                                _ => {}
                            }
                        }
                    }
                    Err(AssessmentReadError::Operational(error)) => return Err(error),
                    _ => {}
                }
            }
        }
        for ((role, edge), (requests, members)) in edge_demand {
            observer(crate::AssessmentEvent::PreparationEdge { role, edge, requests, members });
        }
        Ok(Self {
            packages,
            definitions,
            authored,
            _content: content,
            preparation,
            authored_consumed: RefCell::default(),
            #[cfg(feature = "validator-package-experiment")]
            planned_dependencies: None,
        })
    }

    pub fn effective_bindings(
        &self,
        reference: &HolonReference,
        reader: &ProspectiveDescriptorReader,
    ) -> Result<Vec<EffectiveRelationshipMember>, AssessmentReadError> {
        let definition = self.prepared_definition(reference, reader)?;
        definition.bindings_consumed.set(true);
        definition.bindings.clone()
    }

    pub fn effective_constraints(
        &self,
        reference: &HolonReference,
        reader: &ProspectiveDescriptorReader,
    ) -> Result<Vec<EffectiveRelationshipMember>, AssessmentReadError> {
        let definition = self.prepared_definition(reference, reader)?;
        definition.constraints_consumed.set(true);
        definition.constraints.clone()
    }

    fn prepared_definition(
        &self,
        reference: &HolonReference,
        reader: &ProspectiveDescriptorReader,
    ) -> Result<&PreparedDefinition, AssessmentReadError> {
        let selected = reader.select(reference)?;
        self.definitions
            .get(&content_identity(&selected))
            .ok_or_else(|| HolonError::InvalidState("Definition was not prepared".into()).into())
    }

    pub fn authored(
        &self,
        reference: &HolonReference,
        reader: &ProspectiveDescriptorReader,
    ) -> Result<Vec<(RelationshipName, HolonReference)>, AssessmentReadError> {
        let selected = reader.select(reference)?;
        let identity = content_identity(&selected);
        self.authored_consumed.borrow_mut().insert(identity.clone());
        self.authored
            .get(&identity)
            .ok_or_else(|| {
                HolonError::InvalidState("Schema relationship surface was not prepared".into())
            })?
            .clone()
    }

    pub fn get(
        &self,
        context: &Arc<TransactionContext>,
        root: &HolonReference,
    ) -> Result<&DescriptorPackage, HolonError> {
        let package =
            self.packages.get(&ProspectiveIdentity::for_reference(root, context)?).ok_or_else(
                || HolonError::InvalidState("Descriptor package was not constructed".into()),
            )?;
        package.consumed.set(true);
        Ok(package)
    }

    /// Counts distinct prepared products requested by assessment consumers, including errors.
    pub fn preparation_metrics(&self) -> crate::PreparationMetrics {
        crate::PreparationMetrics {
            contract_consumers: self
                .packages
                .values()
                .filter(|package| package.consumed.get())
                .count(),
            constraint_consumers: self
                .definitions
                .values()
                .filter(|definition| definition.constraints_consumed.get())
                .count(),
            binding_consumers: self
                .definitions
                .values()
                .filter(|definition| definition.bindings_consumed.get())
                .count(),
            authored_surface_consumers: self.authored_consumed.borrow().len(),
            ..self.preparation.clone()
        }
    }

    pub fn finish(&mut self, invalid: &HashSet<ProspectiveIdentity>) {
        for (identity, package) in &mut self.packages {
            package.state = if invalid.contains(identity) || package.contract.is_err() {
                PackageState::Blocked
            } else {
                PackageState::Validated
            };
        }
    }
}

/// Enumerates an effective declared surface without ordinary query-layer discovery.
pub(crate) fn declared_names(
    contract: &ContractContributions,
) -> Result<Vec<RelationshipName>, HolonError> {
    let mut names = Vec::new();
    for contribution in &contract.relationships {
        match contribution.member.property_value(type_names::CorePropertyTypeName::TypeName)? {
            Some(core_types::BaseValue::StringValue(name)) => {
                let name = RelationshipName(name);
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            _ => return Err(HolonError::EmptyField("TypeName".into())),
        }
    }
    names.sort_by_key(ToString::to_string);
    Ok(names)
}
