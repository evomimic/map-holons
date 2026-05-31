use crate::descriptors::{accessor_helpers, Descriptor, HolonDescriptor, TypeHeader};
use crate::reference_layer::HolonReference;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

/// Runtime wrapper for dance response descriptors.
pub struct DanceResponseDescriptor {
    holon: HolonReference,
}

impl DanceResponseDescriptor {
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    pub fn response_body(&self) -> Result<Option<HolonDescriptor>, HolonError> {
        Ok(accessor_helpers::optional_single_related(
            &self.holon,
            CoreRelationshipTypeName::ResponseBody,
        )?
        .map(HolonDescriptor::from_holon))
    }
}

impl From<HolonReference> for DanceResponseDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for DanceResponseDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

