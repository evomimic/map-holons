use crate::descriptors::{Descriptor, TypeHeader};
use crate::reference_layer::HolonReference;

/// Runtime wrapper for property descriptors.
///
/// This remains a thin view in Phase 1/2 so later value-type behavior can land
/// on a stable wrapper without changing call-site types.
pub struct PropertyDescriptor {
    holon: HolonReference,
}

impl PropertyDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }
}

impl From<HolonReference> for PropertyDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for PropertyDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<PropertyDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use base_types::MapString;
    use core_types::HolonError;

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "property-descriptor",
            "PropertyType",
            "Property",
        )?);

        let descriptor = PropertyDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("PropertyType".to_string()));

        Ok(())
    }
}
