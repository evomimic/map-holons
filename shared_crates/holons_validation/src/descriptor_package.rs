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
    collections::{HashMap, HashSet, VecDeque},
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
}

/// Package roots share the same prepared saved memberships and prospective definitions.
/// Entries are discarded before another assessment or schema mutation.
pub(crate) struct DescriptorPackages {
    packages: HashMap<ProspectiveIdentity, DescriptorPackage>,
    definitions: HashMap<ProspectiveIdentity, PreparedDefinition>,
    authored: HashMap<
        ProspectiveIdentity,
        Result<Vec<(RelationshipName, HolonReference)>, AssessmentReadError>,
    >,
    _content: Vec<holons_core::core_shared_objects::holon_cache::SavedHolonRetention>,
}

/// Shared kernel products; configuration and other domain values remain on the holons.
struct PreparedDefinition {
    contract: Result<Arc<ContractContributions>, AssessmentReadError>,
    constraints: Result<Vec<EffectiveRelationshipMember>, AssessmentReadError>,
    bindings: Result<Vec<EffectiveRelationshipMember>, AssessmentReadError>,
}

/// Content identity preserves explicit competing staged candidates without abbreviated labels.
fn content_identity(reference: &HolonReference) -> ProspectiveIdentity {
    match reference {
        HolonReference::Smart(saved) => ProspectiveIdentity::Saved(saved.holon_id()),
        HolonReference::Staged(staged) => ProspectiveIdentity::Staged(staged.temporary_id()),
        HolonReference::Transient(transient) => {
            ProspectiveIdentity::Transient(transient.temporary_id())
        }
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
    pub fn construct(
        context: &Arc<TransactionContext>,
        reader: &ProspectiveDescriptorReader,
        roots: &[HolonReference],
        subjects: &[HolonReference],
        workset: &SchemaWorkset,
    ) -> Result<Self, HolonError> {
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
            if content_seen.insert(identity) {
                definitions_to_prepare.push(selected.clone());
                if let HolonReference::Smart(saved) = &selected {
                    content.push(saved.retain_content()?);
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
            if let HolonReference::Smart(saved) = &selected {
                saved.prepare_relationships(
                    &edges.iter().map(|edge| edge.as_relationship_name()).collect::<Vec<_>>(),
                )?;
            }
            for edge in edges {
                let members = targets(&selected, edge.clone())?;
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
        }
        let mut definitions = HashMap::new();
        for reference in definitions_to_prepare {
            definitions.insert(
                content_identity(&reference),
                PreparedDefinition {
                    contract: ContractContributions::resolve_with_reader(&reference, reader)
                        .map(Arc::new),
                    constraints:
                        holons_core::descriptors::effective_relationship_targets_with_reader(
                            &reference,
                            R::Constraints,
                            reader,
                        ),
                    bindings: holons_core::descriptors::effective_relationship_targets_with_reader(
                        &reference,
                        R::ValidationBindings,
                        reader,
                    ),
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
        Ok(Self { packages, definitions, authored, _content: content })
    }

    pub fn effective_bindings(
        &self,
        reference: &HolonReference,
        reader: &ProspectiveDescriptorReader,
    ) -> Result<Vec<EffectiveRelationshipMember>, AssessmentReadError> {
        self.prepared_definition(reference, reader)?.bindings.clone()
    }

    pub fn effective_constraints(
        &self,
        reference: &HolonReference,
        reader: &ProspectiveDescriptorReader,
    ) -> Result<Vec<EffectiveRelationshipMember>, AssessmentReadError> {
        self.prepared_definition(reference, reader)?.constraints.clone()
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
        self.authored
            .get(&content_identity(&selected))
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
        self.packages.get(&ProspectiveIdentity::for_reference(root, context)?).ok_or_else(|| {
            HolonError::InvalidState("Descriptor package was not constructed".into())
        })
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
