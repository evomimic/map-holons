use crate::descriptors::Descriptor;
use crate::reference_layer::HolonReference;

/// Runtime wrapper for relationship descriptors.
///
/// Relationship-specific structural and inverse-link behavior will accumulate
/// here in later phases while the wrapper itself stays just a typed view.
pub struct RelationshipDescriptor {
    holon: HolonReference,
}

impl RelationshipDescriptor {
    #[allow(dead_code)]
    pub(crate) fn new(holon: HolonReference) -> Self {
        Self { holon }
    }
}

impl Descriptor for RelationshipDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<RelationshipDescriptor>();
};
