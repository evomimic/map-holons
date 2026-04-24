use crate::descriptors::Descriptor;
use crate::reference_layer::HolonReference;

/// Runtime wrapper for value-type descriptors.
///
/// Validation and operator behavior will dispatch through this wrapper in later
/// phases, so the typed shell lands early even while behavior is still deferred.
pub struct ValueDescriptor {
    holon: HolonReference,
}

impl ValueDescriptor {
    #[allow(dead_code)]
    pub(crate) fn new(holon: HolonReference) -> Self {
        Self { holon }
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
