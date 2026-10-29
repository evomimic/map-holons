use super::structural_diagnosis::local_targets;
use super::{has_role, resolve_describing_type, walk_extends_chain, DescribingTypeResolution};
use crate::reference_layer::HolonReference;
use core_types::HolonError;
use type_names::{CoreDefinitionRole, CoreRelationshipTypeName};

/// Authored ownership namespaces must remain separate when reading relationships.
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

/// Resolves local ownership, recognizing Schema on the owner's describing lineage.
/// Missing/ambiguous describing targets are semantic defects. Unreadable targets
/// remain operational errors; readable lineage defects retain their owner identity.
pub fn resolve_schema_ownership(
    subject: &HolonReference,
    kind: SchemaOwnershipKind,
) -> Result<SchemaOwnershipResolution, HolonError> {
    let targets = local_targets(subject, kind.relationship())?;
    let owner = match targets.as_slice() {
        [] => return Ok(SchemaOwnershipResolution::Missing),
        [owner] => owner.clone(),
        _ => return Ok(SchemaOwnershipResolution::Multiple(targets)),
    };
    let describing = resolve_describing_type(&owner)?;
    if let DescribingTypeResolution::Unique(descriptor) = &describing {
        match walk_extends_chain(descriptor).collect::<Result<Vec<_>, _>>() {
            Ok(lineage) => {
                for member in &lineage {
                    if has_role(member, CoreDefinitionRole::Schema)? {
                        return Ok(SchemaOwnershipResolution::OwnedBy(owner));
                    }
                }
                return Ok(SchemaOwnershipResolution::InvalidOwner { owner, describing });
            }
            Err(error) => match &error {
                defect
                @ (HolonError::MultipleExtends { .. } | HolonError::CyclicExtends { .. }) => {
                    return Ok(SchemaOwnershipResolution::MalformedOwnerLineage {
                        owner,
                        defect: defect.clone(),
                    })
                }
                _ => return Err(error),
            },
        }
    }
    Ok(SchemaOwnershipResolution::InvalidOwner { owner, describing })
}

/// Reads authored, local schema dependencies without transitive expansion.
pub fn schema_dependencies(schema: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    local_targets(schema, CoreRelationshipTypeName::DependsOn)
}

/// Reads materialized descriptor membership through the reference layer.
/// These targets do not select a lineage head or expand validation subjects.
pub fn schema_components(schema: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    local_targets(schema, CoreRelationshipTypeName::Components)
}

/// Reads materialized owned rules, separately from descriptor components.
pub fn schema_rules(schema: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    local_targets(schema, CoreRelationshipTypeName::Rules)
}
