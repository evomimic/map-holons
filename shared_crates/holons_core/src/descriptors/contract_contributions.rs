use super::{ancestors, inheritance::relationship_targets_in_lineage, EffectiveRelationshipMember};
use crate::reference_layer::HolonReference;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

/// Additive member namespaces retain distinct identities and contribution provenance.
/// No semantic-name normalization is performed; key-rule overrides are excluded.
#[derive(Clone, Debug)]
pub struct ContractContributions {
    pub properties: Vec<EffectiveRelationshipMember>,
    pub relationships: Vec<EffectiveRelationshipMember>,
}
impl ContractContributions {
    /// Reads the contract defined by this descriptor, preserving contribution provenance.
    pub fn resolve(descriptor: &HolonReference) -> Result<Self, HolonError> {
        let lineage = ancestors(descriptor)?;
        Ok(Self {
            properties: relationship_targets_in_lineage(
                &lineage,
                CoreRelationshipTypeName::InstanceProperties,
            )?,
            relationships: relationship_targets_in_lineage(
                &lineage,
                CoreRelationshipTypeName::InstanceRelationships,
            )?,
        })
    }
}
