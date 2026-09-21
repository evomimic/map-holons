use super::{effective_relationship_targets, EffectiveRelationshipMember};
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
    /// Reads the contract defined by this descriptor, not its governing contract.
    pub fn resolve(descriptor: &HolonReference) -> Result<Self, HolonError> {
        Ok(Self {
            properties: effective_relationship_targets(
                descriptor,
                CoreRelationshipTypeName::InstanceProperties,
            )?,
            relationships: effective_relationship_targets(
                descriptor,
                CoreRelationshipTypeName::InstanceRelationships,
            )?,
        })
    }
}
