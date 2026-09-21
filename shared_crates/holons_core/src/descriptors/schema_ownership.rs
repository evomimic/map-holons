use super::definition_identity::lineage_contains;
use super::structural_diagnosis::local_targets;
use super::{ancestors, resolve_describing_type, DescribingTypeResolution};
use crate::reference_layer::HolonReference;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

/// Authored ownership namespaces must remain separate in prospective collections.
#[derive(Clone, Copy, Debug)]
pub enum SchemaOwnershipKind {
    Component,
    Rule,
}
impl SchemaOwnershipKind {
    fn relationship(self) -> CoreRelationshipTypeName {
        match self {
            Self::Component => CoreRelationshipTypeName::ComponentOf,
            Self::Rule => CoreRelationshipTypeName::RuleOf,
        }
    }
}

/// Readable ownership defects retain all targets for scheduling and diagnostics.
#[derive(Clone, Debug)]
pub enum SchemaOwnershipResolution {
    Missing,
    Multiple(Vec<HolonReference>),
    InvalidOwner { owner: HolonReference, describing: DescribingTypeResolution },
    MalformedOwnerLineage { owner: HolonReference, defect: HolonError },
    OwnedBy(HolonReference),
}

/// Resolves local ownership; the caller supplies the designated Schema type identity.
/// Missing/ambiguous describing targets are semantic defects. Unreadable targets
/// remain operational errors; readable lineage defects retain their owner identity.
pub fn resolve_schema_ownership(
    subject: &HolonReference,
    kind: SchemaOwnershipKind,
    schema_type: &HolonReference,
) -> Result<SchemaOwnershipResolution, HolonError> {
    let targets = local_targets(subject, kind.relationship())?;
    let owner = match targets.as_slice() {
        [] => return Ok(SchemaOwnershipResolution::Missing),
        [owner] => owner.clone(),
        _ => return Ok(SchemaOwnershipResolution::Multiple(targets)),
    };
    let describing = resolve_describing_type(&owner)?;
    if let DescribingTypeResolution::Unique(descriptor) = &describing {
        match ancestors(descriptor) {
            Ok(lineage) if lineage_contains(&lineage, schema_type) => {
                return Ok(SchemaOwnershipResolution::OwnedBy(owner))
            }
            Ok(_) => return Ok(SchemaOwnershipResolution::InvalidOwner { owner, describing }),
            Err(
                defect @ (HolonError::MultipleExtends { .. } | HolonError::CyclicExtends { .. }),
            ) => return Ok(SchemaOwnershipResolution::MalformedOwnerLineage { owner, defect }),
            Err(error) => return Err(error),
        }
    }
    Ok(SchemaOwnershipResolution::InvalidOwner { owner, describing })
}

/// Reads authored, local schema dependencies without transitive expansion.
pub fn schema_dependencies(schema: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    local_targets(schema, CoreRelationshipTypeName::DependsOn)
}

/// Reads materialized descriptor membership through the reference layer.
/// Staged authored membership is composed separately by the prospective view.
pub fn schema_components(schema: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    local_targets(schema, CoreRelationshipTypeName::Components)
}

/// Reads materialized owned rules, separately from descriptor components.
pub fn schema_rules(schema: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    local_targets(schema, CoreRelationshipTypeName::Rules)
}
