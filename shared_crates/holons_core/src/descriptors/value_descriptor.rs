use crate::descriptors::{Descriptor, TypeHeader};
use crate::reference_layer::HolonReference;

/// Runtime wrapper for value-type descriptors.
///
/// Validation and operator behavior will dispatch through this wrapper in later
/// phases, so the typed shell lands early even while behavior is still deferred.
pub struct ValueDescriptor {
    holon: HolonReference,
}

impl ValueDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }
}

impl From<HolonReference> for ValueDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for ValueDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<ValueDescriptor>();
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
            "value-descriptor",
            "StringValueType",
            "Value",
        )?);

        let descriptor = ValueDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("StringValueType".to_string()));

        Ok(())
    }
}
