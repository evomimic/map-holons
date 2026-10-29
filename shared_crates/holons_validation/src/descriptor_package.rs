//! Assessment-local effective contracts. Holon content remains in bound references/caches.
use crate::assessment_support::targets;
use crate::schema_view::SchemaWorkset;
use core_types::{HolonError, RelationshipName};
use holons_core::{
    core_shared_objects::transactions::TransactionContext, ContractContributions,
    DefinitionIdentity, EffectiveRelationshipMember, HolonReference, ReadableHolon,
};
use std::{
    cell::Cell,
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
    pub contract: Result<Arc<ContractContributions>, HolonError>,
    pub state: PackageState,
    consumed: Cell<bool>,
}

/// Package roots share exact prepared definitions.
/// Entries are discarded before another assessment or schema mutation.
pub(crate) struct DescriptorPackages {
    context: Arc<TransactionContext>,
    // Immutable products and their consumption metrics belong to exact definitions.
    packages: HashMap<DefinitionIdentity, DescriptorPackage>,
    definitions: HashMap<DefinitionIdentity, PreparedDefinition>,
    _content: Vec<holons_core::core_shared_objects::holon_cache::SavedHolonRetention>,
    preparation: crate::PreparationMetrics,
}

/// Shared kernel products; configuration and other domain values remain on the holons.
struct PreparedDefinition {
    constraints_consumed: Cell<bool>,
    bindings_consumed: Cell<bool>,
    contract: Result<Arc<ContractContributions>, HolonError>,
    schema: Result<Arc<holons_core::descriptors::ComputedEffectiveSchema>, HolonError>,
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
        roots: &[HolonReference],
        subjects: &[HolonReference],
        workset: &SchemaWorkset,
    ) -> Result<Self, HolonError> {
        Self::construct_with_observer(context, roots, subjects, workset, &mut |_| {})
    }

    pub fn construct_with_observer(
        context: &Arc<TransactionContext>,
        roots: &[HolonReference],
        subjects: &[HolonReference],
        workset: &SchemaWorkset,
        observer: &mut impl FnMut(crate::AssessmentEvent),
    ) -> Result<Self, HolonError> {
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::PackageDiscovery));
        let mut edge_demand: BTreeMap<(&str, String), (usize, usize)> = BTreeMap::new();
        let mut packages = HashMap::new();
        for root in roots {
            let identity = DefinitionIdentity::for_reference(root, context)?;
            packages.entry(identity.clone()).or_insert_with(|| DescriptorPackage {
                root: identity.definition_reference(context, root),
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
        }
        let mut seen = HashSet::new();
        let mut content_seen = HashSet::new();
        let mut definitions_to_prepare = Vec::new();
        let mut content = Vec::new();
        while let Some((reference, role)) = pending.pop_front() {
            // Immutable products belong to definitions, never to a graph-only snapshot.
            let identity = DefinitionIdentity::for_reference(&reference, context)?;
            let selected = identity.definition_reference(context, &reference);
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
                // referenced holon's full contract.
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
            // A schema-edit assessment must observe its candidate graph, including
            // repeated declarations. Saved-only assessments can reuse the shipped snapshot.
            let prepared = if workset.schemas.is_empty() {
                holons_core::descriptors::EffectiveHolonDescriptor::find_in_context(
                    context, &reference,
                )?
            } else {
                None
            };
            let schema = match prepared {
                Some(descriptor) => {
                    holons_core::descriptors::ComputedEffectiveSchema::from_prepared(&descriptor)
                }
                None => holons_core::descriptors::ComputedEffectiveSchema::resolve(&reference),
            }
            .map(Arc::new);
            // All name/navigation reads required by validation belong to preparation,
            // and the same cached projection is consumed during assessment.
            if let Ok(schema) = &schema {
                for vocabulary in [
                    holons_core::descriptors::EffectiveVocabulary::Properties,
                    holons_core::descriptors::EffectiveVocabulary::Dances,
                    holons_core::descriptors::EffectiveVocabulary::Commands,
                ] {
                    let _ = schema.name_index(vocabulary); // Results, including errors, remain cached.
                }
                if schema.defines_holon_instances()? {
                    let _ = schema
                        .name_index(holons_core::descriptors::EffectiveVocabulary::Relationships);
                }
            }
            let contract = schema
                .as_ref()
                .map(|schema| Arc::new(schema.contract_contributions()))
                .map_err(Clone::clone);
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
            observer(crate::AssessmentEvent::PreparationOperation {
                operation: "EffectiveBindings",
                role: "PreparedDefinition",
                starting: false,
            });
            definitions.insert(
                DefinitionIdentity::for_reference(&reference, context)?,
                PreparedDefinition {
                    constraints_consumed: Cell::new(false),
                    bindings_consumed: Cell::new(false),
                    contract,
                    schema,
                },
            );
        }
        for root in roots {
            let identity = DefinitionIdentity::for_reference(root, context)?;
            let entry = packages.get_mut(&identity).expect("root registered before discovery");
            entry.contract = definitions
                .get(&identity)
                .ok_or_else(|| HolonError::InvalidState("Missing prepared descriptor".into()))?
                .contract
                .clone();
            entry.state = PackageState::Constructed;
        }
        for ((role, edge), (requests, members)) in edge_demand {
            observer(crate::AssessmentEvent::PreparationEdge { role, edge, requests, members });
        }
        Ok(Self {
            context: Arc::clone(context),
            packages,
            definitions,
            _content: content,
            preparation,
        })
    }

    pub fn effective_schema(
        &self,
        reference: &HolonReference,
    ) -> Result<&holons_core::descriptors::ComputedEffectiveSchema, HolonError> {
        self.prepared_definition(reference)?.schema.as_ref().map(Arc::as_ref).map_err(Clone::clone)
    }

    pub fn effective_bindings(
        &self,
        reference: &HolonReference,
    ) -> Result<Vec<EffectiveRelationshipMember>, HolonError> {
        let definition = self.prepared_definition(reference)?;
        definition.bindings_consumed.set(true);
        Ok(definition
            .schema
            .as_ref()
            .map_err(Clone::clone)?
            .contributions(R::ValidationBindings)
            .iter()
            .map(|entry| EffectiveRelationshipMember {
                member: entry.member.clone(),
                declared_on: entry.declared_on.clone(),
            })
            .collect())
    }

    pub fn effective_constraints(
        &self,
        reference: &HolonReference,
    ) -> Result<Vec<EffectiveRelationshipMember>, HolonError> {
        let definition = self.prepared_definition(reference)?;
        definition.constraints_consumed.set(true);
        Ok(definition
            .schema
            .as_ref()
            .map_err(Clone::clone)?
            .contributions(R::Constraints)
            .iter()
            .map(|entry| EffectiveRelationshipMember {
                member: entry.member.clone(),
                declared_on: entry.declared_on.clone(),
            })
            .collect())
    }

    fn prepared_definition(
        &self,
        reference: &HolonReference,
    ) -> Result<&PreparedDefinition, HolonError> {
        self.definitions
            .get(&DefinitionIdentity::for_reference(reference, &self.context)?)
            .ok_or_else(|| HolonError::InvalidState("Definition was not prepared".into()).into())
    }

    pub fn get(
        &self,
        context: &Arc<TransactionContext>,
        root: &HolonReference,
    ) -> Result<&DescriptorPackage, HolonError> {
        let package =
            self.packages.get(&DefinitionIdentity::for_reference(root, context)?).ok_or_else(
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
            ..self.preparation.clone()
        }
    }

    pub fn finish(&mut self, invalid: &HashSet<DefinitionIdentity>) {
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
