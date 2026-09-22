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

/// One configured constraint with concrete type and attachment provenance.
///
/// Construction requires an unambiguous describing type. Unlike validating this
/// holon as a subject, consuming it as a commitment cannot continue without that
/// execution dependency: descriptor-resolution failures return `HolonError`.
/// A resolved but unsupported concrete type instead produces a blocking finding.
pub struct ResolvedConstraint {
    /// Configured constraint instance; parameters are not reconstructed elsewhere.
    pub constraint: HolonReference,
    /// Concrete describing type used by static constraint dispatch.
    pub constraint_type: HolonDescriptor,
    /// Descriptor that contributed this occurrence to the effective contract.
    pub declaring_descriptor: HolonDescriptor,
}

impl TryFrom<EffectiveRelationshipMember> for ResolvedConstraint {
    type Error = HolonError;

    fn try_from(contribution: EffectiveRelationshipMember) -> Result<Self, Self::Error> {
        let constraint_type = contribution.member.holon_descriptor()?;
        Ok(Self {
            constraint: contribution.member,
            constraint_type,
            declaring_descriptor: HolonDescriptor::from_holon(contribution.declared_on),
        })
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

impl ResolvedConstraint {
    /// Resolves configured parameters and their describing type from one prospective view.
    /// This is declaration preparation, not instance constraint evaluation.
    pub fn with_reader<R: holons_core::DescriptorReader>(
        contribution: EffectiveRelationshipMember,
        reader: &R,
    ) -> Result<Self, R::Error> {
        use holons_core::DescribingTypeResolution;
        let constraint = reader.select(&contribution.member)?;
        let constraint_type = match holons_core::descriptors::resolve_describing_type_with_reader(
            &constraint,
            reader,
        )? {
            DescribingTypeResolution::Unique(descriptor) => descriptor,
            DescribingTypeResolution::Missing => {
                return Err(HolonError::MissingDescribedBy {
                    holon: constraint.reference_id_string(),
                }
                .into())
            }
            DescribingTypeResolution::Multiple(targets) => {
                return Err(HolonError::MultipleDescribedBy {
                    holon: constraint.reference_id_string(),
                    count: targets.len(),
                }
                .into())
            }
        };
        Ok(Self {
            constraint,
            constraint_type: HolonDescriptor::from_holon(constraint_type),
            declaring_descriptor: HolonDescriptor::from_holon(
                reader.select(&contribution.declared_on)?,
            ),
        })
    }
}
