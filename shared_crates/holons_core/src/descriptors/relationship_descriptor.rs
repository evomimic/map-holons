use crate::descriptors::Descriptor;
use crate::reference_layer::HolonReference;

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
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<RelationshipDescriptor>();
};
