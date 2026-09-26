use super::{
    effective_relationship_targets_with_reader, CurrentDescriptorReader, DescriptorReader,
    EffectiveRelationshipMember,
};
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
        Self::resolve_with_reader(descriptor, &CurrentDescriptorReader)
    }

    /// Preserves contribution provenance while selecting each prospective definition.
    pub fn resolve_with_reader<R: DescriptorReader>(
        descriptor: &HolonReference,
        reader: &R,
    ) -> Result<Self, R::Error> {
        Ok(Self {
            properties: effective_relationship_targets_with_reader(
                descriptor,
                CoreRelationshipTypeName::InstanceProperties,
                reader,
            )?,
            relationships: effective_relationship_targets_with_reader(
                descriptor,
                CoreRelationshipTypeName::InstanceRelationships,
                reader,
            )?,
        })
    }
}
