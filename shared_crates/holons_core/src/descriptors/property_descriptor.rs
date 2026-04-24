use crate::descriptors::Descriptor;
use crate::reference_layer::HolonReference;

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
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<PropertyDescriptor>();
};
