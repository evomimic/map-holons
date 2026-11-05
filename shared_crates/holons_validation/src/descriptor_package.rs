//! Assessment-local effective contracts. Holon content remains in bound references/caches.
use crate::assessment_support::targets;
use crate::schema_view::SchemaWorkset;
use core_types::HolonError;
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

/// The root's effective contract retains kernel contribution ordering and provenance.
pub(crate) struct DescriptorPackage {
    pub root: HolonReference,
    pub contract: Result<Arc<ContractContributions>, HolonError>,
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
    pub(crate) diagnoses: Arc<crate::structural_diagnoses::StructuralDiagnoses>,
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
        Self::construct_with_observer(
            context,
            roots,
            subjects,
            workset,
            Arc::new(crate::structural_diagnoses::StructuralDiagnoses::new(context)),
            &mut |_| {},
        )
    }

    pub fn construct_with_observer(
        context: &Arc<TransactionContext>,
        roots: &[HolonReference],
        subjects: &[HolonReference],
        workset: &SchemaWorkset,
        diagnoses: Arc<crate::structural_diagnoses::StructuralDiagnoses>,
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
                    let operation = crate::assessment_phase::observe_operation_scope(
                        observer,
                        "ContentRetention",
                        "SavedContent",
                    );
                    let retained = saved.retain_content();
                    drop(operation);
                    content.push(retained?);
                }
            }
            // Preserve the current preparation closure over definition relationships.
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
                let operation = crate::assessment_phase::observe_operation_scope(
                    observer,
                    "MembershipPrefetch",
                    role_name,
                );
                let preparation = saved.prepare_relationships(
                    &edges.iter().map(|edge| edge.as_relationship_name()).collect::<Vec<_>>(),
                );
                drop(operation);
                preparation?;
            }
            let operation = crate::assessment_phase::observe_operation_scope(
                observer,
                "RelationshipTraversal",
                role_name,
            );
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
            drop(operation);
            observer(crate::AssessmentEvent::PreparationVisitFinished {
                type_name,
                distinct_content,
            });
        }
        let mut preparation = crate::PreparationMetrics {
            package_roots: packages.len(),
            subjects: subjects.len(),
            affected_schemas: workset.schemas.len(),
            distinct_content: definitions_to_prepare.len(),
            requested_schema_products: definitions_to_prepare.len(),
            identity_role_visits: seen.len(),
            ..Default::default()
        };
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::PreparedProducts));
        let mut definitions = HashMap::new();
        for reference in definitions_to_prepare {
            let prepared = crate::assessment_phase::observe_operation(
                observer,
                "RegistryLookup",
                "PreparedDefinition",
                || {
                    holons_core::descriptors::EffectiveHolonDescriptor::find_in_context(
                        context, &reference,
                    )
                },
            )?;
            let schema = match prepared {
                Some(descriptor) => crate::assessment_phase::observe_operation(
                    observer,
                    "SavedRestoration",
                    "PreparedDefinition",
                    || {
                        holons_core::descriptors::ComputedEffectiveSchema::from_prepared(&descriptor)
                    },
                ),
                None => {
                    let diagnosis = crate::assessment_phase::observe_operation(
                        observer,
                        "StructuralDiagnosis",
                        "CategoryRouting",
                        || diagnoses.get(&reference),
                    )?;
                    match diagnosis.valid_lineage() {
                        None => Err(diagnosed_lineage_error(&diagnosis, &reference)),
                        Some(lineage) => {
                            use type_names::CoreDefinitionRole as Role;
                            if lineage.contains_role(Role::ValueType) {
                                context.retain_frozen_descriptor_lineage(lineage)?;
                                crate::assessment_phase::observe_operation(
                                    observer,
                                    "DerivedAcquisition",
                                    "ValueType",
                                    || {
                                        holons_core::ValueDescriptor::from_holon(reference.clone())
                                            .effective_schema()
                                    },
                                )
                            } else if lineage.contains_role(Role::PropertyType) {
                                context.retain_frozen_descriptor_lineage(lineage)?;
                                crate::assessment_phase::observe_operation(
                                    observer,
                                    "DerivedAcquisition",
                                    "PropertyType",
                                    || {
                                        holons_core::PropertyDescriptor::from_holon(reference.clone())
                                            .effective_schema()
                                    },
                                )
                            } else {
                                crate::assessment_phase::observe_operation(
                                    observer,
                                    "FreshEffectiveSchema",
                                    "PreparedDefinition",
                                    || {
                                        holons_core::descriptors::ComputedEffectiveSchema::from_valid_lineage(lineage)
                                    },
                                )
                            }
                        }
                    }
                }
            }
            .map(Arc::new);
            // All name/navigation reads required by validation belong to preparation,
            // and the same cached projection is consumed during assessment.
            crate::assessment_phase::observe_operation(
                observer,
                "NameIndexes",
                "PreparedDefinition",
                || -> Result<(), HolonError> {
                    if let Ok(schema) = &schema {
                        for vocabulary in [
                            holons_core::descriptors::EffectiveVocabulary::Properties,
                            holons_core::descriptors::EffectiveVocabulary::Dances,
                            holons_core::descriptors::EffectiveVocabulary::Commands,
                        ] {
                            let _ = schema.name_index(vocabulary); // Results, including errors, remain cached.
                        }
                        if schema.defines_holon_instances()? {
                            let _ = schema.name_index(
                                holons_core::descriptors::EffectiveVocabulary::Relationships,
                            );
                        }
                    }
                    Ok(())
                },
            )?;
            let contract = crate::assessment_phase::observe_operation(
                observer,
                "ContributionProjection",
                "PreparedDefinition",
                || {
                    schema
                        .as_ref()
                        .map(|schema| Arc::new(schema.contract_contributions()))
                        .map_err(Clone::clone)
                },
            );
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
        preparation.constructed_schema_products =
            definitions.values().filter(|definition| definition.schema.is_ok()).count();
        for root in roots {
            let identity = DefinitionIdentity::for_reference(root, context)?;
            let entry = packages.get_mut(&identity).expect("root registered before discovery");
            entry.contract = definitions
                .get(&identity)
                .ok_or_else(|| HolonError::InvalidState("Missing prepared descriptor".into()))?
                .contract
                .clone();
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
            diagnoses,
        })
    }

    /// Supplies reached binding contributions for construction-time role preparation.
    pub fn binding_contributions(&self) -> Vec<EffectiveRelationshipMember> {
        self.definitions
            .values()
            .filter_map(|definition| definition.schema.as_ref().ok())
            .flat_map(|schema| schema.contributions(R::ValidationBindings))
            .map(|entry| EffectiveRelationshipMember {
                member: entry.member.clone(),
                declared_on: entry.declared_on.clone(),
            })
            .collect()
    }

    /// Projects obligations from complete prepared occurrences without another collection walk.
    pub fn constraint_contributions(
        &self,
        reference: &HolonReference,
    ) -> Result<holons_core::descriptors::ConstraintContributions, HolonError> {
        let diagnosis = self.diagnoses.get(reference)?;
        let lineage = diagnosis.valid_lineage().ok_or_else(|| {
            HolonError::InvalidState(format!(
                "Constraint contributions require a valid lineage for {}",
                reference.reference_id_string()
            ))
        })?;
        let parent = lineage.members().get(1).cloned();
        let effective = self.effective_constraints(reference)?;
        let inherited = match &parent {
            Some(parent) => self.effective_constraints(parent)?,
            None => Vec::new(),
        };
        Ok(holons_core::descriptors::ConstraintContributions { parent, inherited, effective })
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
}

/// Preserve typed structural defects separately from missing promised preparation.
fn diagnosed_lineage_error(
    diagnosis: &holons_core::ExtendsLineageDiagnosis,
    reference: &HolonReference,
) -> HolonError {
    use holons_core::ExtendsLineageDefect;
    match &diagnosis.defects[0] {
        ExtendsLineageDefect::MultipleParents { subject, count } => {
            HolonError::MultipleExtends { descriptor: subject.reference_id_string(), count: *count }
        }
        ExtendsLineageDefect::Cycle { repeated_descriptor, .. } => {
            HolonError::CyclicExtends { descriptor: repeated_descriptor.clone() }
        }
        ExtendsLineageDefect::WrongTermination { terminal } => HolonError::WrongDescriptorKind {
            expected: "Extends lineage terminating at TypeDescriptor".into(),
            found: terminal.reference_id_string(),
            descriptor: reference.reference_id_string(),
        },
        ExtendsLineageDefect::RootHasParent => HolonError::InvalidParameter(
            "Diagnosed TypeDescriptor root has an Extends parent".into(),
        ),
    }
}
