use super::definition_identity::lineage_contains;
use super::structural_diagnosis::{local_targets, local_targets_with_reader};
use super::{
    resolve_describing_type_with_reader, walk_extends_chain_with_reader, CurrentDescriptorReader,
    DescribingTypeResolution, DescriptorReader,
};
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
    resolve_schema_ownership_with_reader(subject, kind, schema_type, &CurrentDescriptorReader)
}

/// Resolves ownership from the same selected content used by other commitment checks.
pub fn resolve_schema_ownership_with_reader<R: DescriptorReader>(
    subject: &HolonReference,
    kind: SchemaOwnershipKind,
    schema_type: &HolonReference,
    reader: &R,
) -> Result<SchemaOwnershipResolution, R::Error> {
    let schema_type = reader.select(schema_type)?;
    let targets = local_targets_with_reader(subject, kind.relationship(), reader)?;
    let owner = match targets.as_slice() {
        [] => return Ok(SchemaOwnershipResolution::Missing),
        [owner] => owner.clone(),
        _ => return Ok(SchemaOwnershipResolution::Multiple(targets)),
    };
    let describing = resolve_describing_type_with_reader(&owner, reader)?;
    if let DescribingTypeResolution::Unique(descriptor) = &describing {
        match walk_extends_chain_with_reader(descriptor, reader).collect::<Result<Vec<_>, _>>() {
            Ok(lineage) if lineage_contains(&lineage, &schema_type) => {
                return Ok(SchemaOwnershipResolution::OwnedBy(owner))
            }
            Ok(_) => return Ok(SchemaOwnershipResolution::InvalidOwner { owner, describing }),
            Err(error) => match R::operational_error(&error) {
                Some(
                    defect
                    @ (HolonError::MultipleExtends { .. } | HolonError::CyclicExtends { .. }),
                ) => {
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
/// Staged authored membership is composed separately by the prospective view.
pub fn schema_components(schema: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    local_targets(schema, CoreRelationshipTypeName::Components)
}

/// Reads materialized owned rules, separately from descriptor components.
pub fn schema_rules(schema: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    local_targets(schema, CoreRelationshipTypeName::Rules)
}

/// Reads prospective schema dependencies without merging ownership namespaces.
pub fn schema_dependencies_with_reader<R: DescriptorReader>(
    schema: &HolonReference,
    reader: &R,
) -> Result<Vec<HolonReference>, R::Error> {
    local_targets_with_reader(schema, CoreRelationshipTypeName::DependsOn, reader)
}

/// Reads prospective schema components without merging ownership namespaces.
pub fn schema_components_with_reader<R: DescriptorReader>(
    schema: &HolonReference,
    reader: &R,
) -> Result<Vec<HolonReference>, R::Error> {
    local_targets_with_reader(schema, CoreRelationshipTypeName::Components, reader)
}

/// Reads prospective schema rules without merging ownership namespaces.
pub fn schema_rules_with_reader<R: DescriptorReader>(
    schema: &HolonReference,
    reader: &R,
) -> Result<Vec<HolonReference>, R::Error> {
    local_targets_with_reader(schema, CoreRelationshipTypeName::Rules, reader)
}
