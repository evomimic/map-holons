use core_types::HolonError;
use holons_core::{
    Descriptor, EffectiveRelationshipMember, HolonDescriptor, HolonReference, ReadableHolon,
};

/// One effective binding, retaining the descriptor that authored its occurrence.
pub struct ResolvedValidationBinding {
    /// Bound rule holon, resolved by the descriptor runtime.
    pub rule: HolonReference,
    /// The ancestor or local descriptor on which this binding was declared.
    pub declaring_descriptor: HolonDescriptor,
}

impl From<EffectiveRelationshipMember> for ResolvedValidationBinding {
    fn from(contribution: EffectiveRelationshipMember) -> Self {
        Self {
            rule: contribution.member,
            declaring_descriptor: HolonDescriptor::from_holon(contribution.declared_on),
        }
    }
}

pub(crate) fn required_key(reference: &HolonReference) -> Result<String, HolonError> {
    reference.key()?.map(|key| key.to_string()).ok_or_else(|| {
        HolonError::EmptyField(format!(
            "Key on validation commitment {}",
            reference.reference_id_string()
        ))
    })
}

pub(crate) fn declaring_identity(binding: &ResolvedValidationBinding) -> String {
    binding.declaring_descriptor.holon().reference_id_string()
}
