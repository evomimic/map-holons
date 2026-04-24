use crate::descriptors::Descriptor;
use crate::reference_layer::HolonReference;

/// Runtime wrapper for property descriptors.
///
/// This remains a thin view in Phase 1/2 so later value-type behavior can land
/// on a stable wrapper without changing call-site types.
pub struct PropertyDescriptor {
    holon: HolonReference,
}

impl PropertyDescriptor {
    #[allow(dead_code)]
    pub(crate) fn new(holon: HolonReference) -> Self {
        Self { holon }
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
